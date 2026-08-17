//! Windows Imaging Component thumbnail extraction.
//!
//! The importer must never materialize a full-resolution source image. WIC
//! keeps the decoder and scaler in the OS codec pipeline and returns only the
//! requested bounded pixel buffer to Rust.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use image::RgbaImage;
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICBitmapFrameDecode,
    IWICImagingFactory, IWICPalette, WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::core::PCWSTR;

const MAX_SOURCE_DIMENSION: u32 = 16_384;

pub(crate) struct DecodedThumbnail {
    pub(crate) image: RgbaImage,
    pub(crate) raw_width: u32,
    pub(crate) raw_height: u32,
}

pub(crate) fn decode_thumbnail(
    source_path: &Path,
    source_bytes: Option<&[u8]>,
    max_dimension: u32,
) -> Result<DecodedThumbnail, String> {
    let _com = ComGuard::initialize()?;
    let factory: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
            .map_err(|error| format!("WIC factory initialization failed: {error}"))?;

    let decoder = match source_bytes {
        Some(bytes) => {
            let stream = unsafe { factory.CreateStream() }
                .map_err(|error| format!("WIC memory stream creation failed: {error}"))?;
            unsafe { stream.InitializeFromMemory(bytes) }
                .map_err(|error| format!("WIC memory stream initialization failed: {error}"))?;
            match unsafe {
                factory.CreateDecoderFromStream(
                    &stream,
                    std::ptr::null(),
                    WICDecodeMetadataCacheOnDemand,
                )
            } {
                Ok(decoder) => decoder,
                Err(memory_error) => {
                    // Some installed WIC codecs (notably optional WebP
                    // codecs) reject an in-memory stream while accepting a
                    // file-backed stream. Both paths remain bounded: the
                    // fallback still asks WIC for only the target thumbnail.
                    let path = wide_path(source_path);
                    unsafe {
                        factory.CreateDecoderFromFilename(
                            PCWSTR(path.as_ptr()),
                            None,
                            GENERIC_READ,
                            WICDecodeMetadataCacheOnDemand,
                        )
                    }
                    .map_err(|file_error| {
                        format!(
                            "WIC decoder creation failed for memory stream ({memory_error}) and file ({file_error})"
                        )
                    })?
                }
            }
        }
        None => {
            let path = wide_path(source_path);
            unsafe {
                factory.CreateDecoderFromFilename(
                    PCWSTR(path.as_ptr()),
                    None,
                    GENERIC_READ,
                    WICDecodeMetadataCacheOnDemand,
                )
            }
            .map_err(|error| format!("WIC file decoder creation failed: {error}"))?
        }
    };

    let frame = unsafe { decoder.GetFrame(0) }
        .map_err(|error| format!("WIC frame access failed: {error}"))?;
    let (raw_width, raw_height) = frame_size(&frame)?;
    if raw_width > MAX_SOURCE_DIMENSION || raw_height > MAX_SOURCE_DIMENSION {
        return Err(format!(
            "image dimensions exceed the safe import limit of {MAX_SOURCE_DIMENSION} px: {raw_width}x{raw_height}"
        ));
    }
    let (width, height) = bounded_size(raw_width, raw_height, max_dimension);
    let image = scale_and_convert(&factory, &frame, width, height)?;

    Ok(DecodedThumbnail {
        image,
        raw_width,
        raw_height,
    })
}

fn frame_size(frame: &IWICBitmapFrameDecode) -> Result<(u32, u32), String> {
    let mut width = 0;
    let mut height = 0;
    unsafe { frame.GetSize(&mut width, &mut height) }
        .map_err(|error| format!("WIC source dimension read failed: {error}"))?;
    if width == 0 || height == 0 {
        return Err("WIC source returned an empty image".into());
    }
    Ok((width, height))
}

fn bounded_size(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    let max_dimension = max_dimension.max(1);
    let scale = (f64::from(max_dimension) / f64::from(width))
        .min(f64::from(max_dimension) / f64::from(height))
        .min(1.0);
    (
        (f64::from(width) * scale).round().max(1.0) as u32,
        (f64::from(height) * scale).round().max(1.0) as u32,
    )
}

fn scale_and_convert(
    factory: &IWICImagingFactory,
    frame: &IWICBitmapFrameDecode,
    width: u32,
    height: u32,
) -> Result<RgbaImage, String> {
    let scaler = unsafe { factory.CreateBitmapScaler() }
        .map_err(|error| format!("WIC scaler creation failed: {error}"))?;
    unsafe { scaler.Initialize(frame, width, height, WICBitmapInterpolationModeFant) }
        .map_err(|error| format!("WIC bounded scaler initialization failed: {error}"))?;

    let converter = unsafe { factory.CreateFormatConverter() }
        .map_err(|error| format!("WIC pixel converter creation failed: {error}"))?;
    unsafe {
        converter.Initialize(
            &scaler,
            &GUID_WICPixelFormat32bppRGBA,
            WICBitmapDitherTypeNone,
            None::<&IWICPalette>,
            0.0,
            WICBitmapPaletteTypeCustom,
        )
    }
    .map_err(|error| format!("WIC RGBA conversion failed: {error}"))?;

    let stride = width
        .checked_mul(4)
        .ok_or_else(|| "WIC thumbnail stride overflow".to_string())?;
    let buffer_size = stride
        .checked_mul(height)
        .ok_or_else(|| "WIC thumbnail buffer size overflow".to_string())?;
    let mut pixels = vec![
        0u8;
        usize::try_from(buffer_size).map_err(|_| {
            "WIC thumbnail buffer is too large for this process".to_string()
        })?
    ];
    unsafe { converter.CopyPixels(std::ptr::null(), stride, &mut pixels) }
        .map_err(|error| format!("WIC bounded pixel copy failed: {error}"))?;
    RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| "WIC returned an invalid RGBA thumbnail buffer".into())
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

struct ComGuard {
    initialized: bool,
}

impl ComGuard {
    fn initialize() -> Result<Self, String> {
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result.is_ok() {
            Ok(Self { initialized: true })
        } else {
            Err(format!("COM initialization failed: {result}"))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}
