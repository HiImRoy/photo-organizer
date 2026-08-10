use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use exif::{In, Tag, Value};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, GenericImageView, RgbaImage};

use crate::error::{AppError, AppResult};
use crate::models::{BasicImageFeatures, ExifMetadata, ImageProcessingTimings, ProcessedImage};

pub const THUMBNAIL_SPEC: &str = "grid-640-v1";
pub const ANALYSIS_VERSION: &str = "basic-color-v3";
pub const SCREEN_PREVIEW_SPEC: &str = "screen-2560-v1";

pub fn process_image(
    source_path: &Path,
    thumbnail_dir: &Path,
    cache_key: &str,
) -> AppResult<ProcessedImage> {
    process_image_with_source_bytes(source_path, thumbnail_dir, cache_key, None)
}

pub fn process_image_with_source_bytes(
    source_path: &Path,
    thumbnail_dir: &Path,
    cache_key: &str,
    source_bytes: Option<&[u8]>,
) -> AppResult<ProcessedImage> {
    let exif_started = Instant::now();
    let source_metadata = source_bytes
        .map(read_source_metadata_bytes)
        .unwrap_or_else(|| read_source_metadata(source_path));
    let exif = source_metadata.exif.clone();
    let exif_us = elapsed_us(exif_started);

    let decode_started = Instant::now();
    let (decoded, used_embedded_thumbnail) = match source_metadata.embedded_thumbnail.as_deref() {
        Some(bytes) => match image::load_from_memory(bytes) {
            Ok(image) => (image, true),
            Err(_) => (decode_source_image(source_path, source_bytes)?, false),
        },
        None => (decode_source_image(source_path, source_bytes)?, false),
    };
    let decode_us = elapsed_us(decode_started);
    let source_decode_us = if used_embedded_thumbnail {
        0
    } else {
        decode_us
    };
    let thumbnail_decode_us = if used_embedded_thumbnail {
        decode_us
    } else {
        0
    };

    let resize_started = Instant::now();
    let oriented = apply_orientation(decoded, exif.orientation);
    let (width, height) = if used_embedded_thumbnail {
        source_dimensions(source_path, source_bytes, exif.orientation)?
    } else {
        oriented.dimensions()
    };
    let thumbnail = bounded_thumbnail(&oriented, 640, 640).to_rgba8();
    let resize_us = elapsed_us(resize_started);

    let analysis_step = if thumbnail.width() > 320 || thumbnail.height() > 320 {
        2
    } else {
        1
    };
    let analysis_started = Instant::now();
    let features = analyze_rgba_with_step(&thumbnail, analysis_step);
    let feature_analysis_us = elapsed_us(analysis_started);

    let thumbnail_write_started = Instant::now();
    fs::create_dir_all(thumbnail_dir)?;
    let thumbnail_path = thumbnail_dir.join(format!("{cache_key}-{THUMBNAIL_SPEC}.jpg"));
    write_thumbnail_once(&thumbnail, &thumbnail_path)?;
    let thumbnail_write_us = elapsed_us(thumbnail_write_started);

    Ok(ProcessedImage {
        width,
        height,
        exif,
        thumbnail_path: path_to_string(&thumbnail_path),
        features,
        timings: ImageProcessingTimings {
            exif_us,
            source_dimension_us: 0,
            decode_us,
            source_decode_us,
            thumbnail_decode_us,
            resize_us,
            feature_analysis_us,
            thumbnail_write_us,
        },
    })
}

/// Re-analyze an existing current cache without decoding source pixels.
///
/// The source is consulted only for EXIF and dimensions. All pixel work comes
/// from the application-owned `grid-640-v1` thumbnail. This path is used when
/// the basic feature algorithm changes or an interrupted import left a valid
/// thumbnail behind.
pub fn process_image_from_cached_thumbnail(
    source_path: &Path,
    thumbnail_path: &Path,
) -> AppResult<ProcessedImage> {
    let exif_started = Instant::now();
    let exif = read_source_metadata(source_path).exif;
    let exif_us = elapsed_us(exif_started);

    let dimension_started = Instant::now();
    let (width, height) = source_dimensions(source_path, None, exif.orientation)?;
    let source_dimension_us = elapsed_us(dimension_started);

    let decode_started = Instant::now();
    let thumbnail = image::ImageReader::open(thumbnail_path)?
        .with_guessed_format()?
        .decode()?
        .to_rgba8();
    let thumbnail_decode_us = elapsed_us(decode_started);

    let analysis_step = if thumbnail.width() > 320 || thumbnail.height() > 320 {
        2
    } else {
        1
    };
    let analysis_started = Instant::now();
    let features = analyze_rgba_with_step(&thumbnail, analysis_step);
    let feature_analysis_us = elapsed_us(analysis_started);

    Ok(ProcessedImage {
        width,
        height,
        exif,
        thumbnail_path: path_to_string(thumbnail_path),
        features,
        timings: ImageProcessingTimings {
            exif_us,
            source_dimension_us,
            decode_us: thumbnail_decode_us,
            source_decode_us: 0,
            thumbnail_decode_us,
            resize_us: 0,
            feature_analysis_us,
            thumbnail_write_us: 0,
        },
    })
}

fn decode_source_image(source_path: &Path, source_bytes: Option<&[u8]>) -> AppResult<DynamicImage> {
    match source_bytes {
        Some(bytes) => Ok(image::load_from_memory(bytes)?),
        None => Ok(image::ImageReader::open(source_path)?
            .with_guessed_format()?
            .decode()?),
    }
}

fn source_dimensions(
    source_path: &Path,
    source_bytes: Option<&[u8]>,
    orientation: u32,
) -> AppResult<(u32, u32)> {
    let is_jpeg = source_path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "jpg" | "jpeg"));

    let raw_dimensions = if is_jpeg {
        source_bytes
            .and_then(jpeg_dimensions)
            .or_else(|| jpeg_dimensions_from_path(source_path))
    } else {
        None
    };

    let raw_dimensions = match raw_dimensions {
        Some(dimensions) => dimensions,
        None => match source_bytes {
            Some(bytes) => image::ImageReader::new(Cursor::new(bytes))
                .with_guessed_format()?
                .into_dimensions()?,
            None => image::ImageReader::open(source_path)?
                .with_guessed_format()?
                .into_dimensions()?,
        },
    };
    Ok(oriented_dimensions(raw_dimensions, orientation))
}

fn jpeg_dimensions_from_path(path: &Path) -> Option<(u32, u32)> {
    let file = File::open(path).ok()?;
    let mut bytes = Vec::new();
    BufReader::new(file)
        .take(1024 * 1024)
        .read_to_end(&mut bytes)
        .ok()?;
    jpeg_dimensions(&bytes)
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }

    let mut offset = 2_usize;
    while offset + 1 < bytes.len() {
        while offset < bytes.len() && bytes[offset] != 0xff {
            offset += 1;
        }
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if matches!(marker, 0xd8 | 0xd9 | 0x01 | 0xd0..=0xd7) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
        ]));
        if length < 2 || offset + length > bytes.len() {
            return None;
        }
        if is_jpeg_start_of_frame(marker) {
            let data_start = offset + 2;
            let height = u32::from(u16::from_be_bytes([
                *bytes.get(data_start + 1)?,
                *bytes.get(data_start + 2)?,
            ]));
            let width = u32::from(u16::from_be_bytes([
                *bytes.get(data_start + 3)?,
                *bytes.get(data_start + 4)?,
            ]));
            return (width > 0 && height > 0).then_some((width, height));
        }
        offset += length;
    }
    None
}

fn is_jpeg_start_of_frame(marker: u8) -> bool {
    matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf)
}

fn oriented_dimensions((width, height): (u32, u32), orientation: u32) -> (u32, u32) {
    if matches!(orientation, 5..=8) {
        (height, width)
    } else {
        (width, height)
    }
}

fn elapsed_us(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

/// Decode a source image with the same EXIF orientation used by thumbnails.
/// The returned image is kept in the caller so the desktop IPC layer can
/// choose an appropriate preview tier without ever modifying the source.
pub fn load_oriented_image(source_path: &Path) -> AppResult<DynamicImage> {
    let exif = read_exif(source_path);
    let decoded = image::ImageReader::open(source_path)?
        .with_guessed_format()?
        .decode()?;
    Ok(apply_orientation(decoded, exif.orientation))
}

fn write_thumbnail_once(image: &RgbaImage, target: &Path) -> AppResult<()> {
    if target.is_file() {
        return Ok(());
    }

    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, 84).encode_image(image)?;

    match OpenOptions::new().write(true).create_new(true).open(target) {
        Ok(mut file) => {
            file.write_all(&encoded)?;
            // Thumbnails are rebuildable application cache. Avoid forcing a
            // physical disk flush for every source file during import.
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(AppError::Io(error)),
    }
    Ok(())
}

fn bounded_thumbnail(image: &DynamicImage, max_width: u32, max_height: u32) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return image.clone();
    }

    let scale = (f64::from(max_width) / f64::from(width))
        .min(f64::from(max_height) / f64::from(height))
        .min(1.0);
    let width = (f64::from(width) * scale).round().max(1.0) as u32;
    let height = (f64::from(height) * scale).round().max(1.0) as u32;
    image.thumbnail_exact(width, height)
}

pub fn analyze_rgba(image: &RgbaImage) -> BasicImageFeatures {
    analyze_rgba_with_step(image, 1)
}

fn analyze_rgba_with_step(image: &RgbaImage, sample_step: u32) -> BasicImageFeatures {
    let (left, top, right, bottom) = analysis_bounds(image);
    let sample_step = sample_step.max(1) as usize;
    let mut brightness = Vec::new();
    let mut saturation = Vec::new();
    let mut color_bins: HashMap<u16, (u64, u64, u64, u64)> = HashMap::new();
    let mut weighted_color_bins: HashMap<u16, (f64, f64, f64, f64)> = HashMap::new();
    let mut hue_histogram = [0u64; 12];
    let mut warm_sum = 0.0;
    let mut warm_weight = 0.0;
    let mut neutral_count = 0u64;
    let mut chromatic_weight_total = 0.0;
    let mut chroma_sum = 0.0;
    let mut rg_sum = 0.0;
    let mut rg_sq_sum = 0.0;
    let mut yb_sum = 0.0;
    let mut yb_sq_sum = 0.0;

    for y in (top..bottom).step_by(sample_step) {
        for x in (left..right).step_by(sample_step) {
            let pixel = image.get_pixel(x, y).0;
            if pixel[3] < 16 {
                continue;
            }

            let r = f64::from(pixel[0]) / 255.0;
            let g = f64::from(pixel[1]) / 255.0;
            let b = f64::from(pixel[2]) / 255.0;
            let light = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            let (sat, hue) = saturation_and_hue(r, g, b);
            let chroma = (r.max(g).max(b) - r.min(g).min(b)).clamp(0.0, 1.0);
            chroma_sum += chroma;
            brightness.push(light);
            saturation.push(sat);

            let key = (u16::from(pixel[0] >> 4) << 8)
                | (u16::from(pixel[1] >> 4) << 4)
                | u16::from(pixel[2] >> 4);
            let entry = color_bins.entry(key).or_default();
            entry.0 += 1;
            entry.1 += u64::from(pixel[0]);
            entry.2 += u64::from(pixel[1]);
            entry.3 += u64::from(pixel[2]);

            if sat < 0.14 || chroma < 0.08 {
                neutral_count += 1;
            } else {
                let bucket = ((hue / 30.0).floor() as usize).min(11);
                hue_histogram[bucket] += 1;
                warm_sum += (hue - 30.0).to_radians().cos() * sat;
                warm_weight += sat;

                // Saturated pixels are the useful signal for a representative
                // colour. Keep dark/highlight pixels in the competition, but
                // reduce their influence instead of letting silhouettes or
                // clipped whites dominate the result.
                let shadow_weight = (0.28 + (light / 0.42).clamp(0.0, 1.0) * 0.72).clamp(0.28, 1.0);
                let highlight_weight =
                    (0.28 + ((1.0 - light) / 0.28).clamp(0.0, 1.0) * 0.72).clamp(0.28, 1.0);
                let weight = sat.powf(0.72) * chroma.sqrt() * shadow_weight * highlight_weight;
                if light > 0.015 && weight > 0.02 {
                    let weighted = weighted_color_bins.entry(key).or_default();
                    weighted.0 += weight;
                    weighted.1 += weight * f64::from(pixel[0]);
                    weighted.2 += weight * f64::from(pixel[1]);
                    weighted.3 += weight * f64::from(pixel[2]);
                    chromatic_weight_total += weight;
                }
            }

            let rg = r - g;
            let yb = 0.5 * (r + g) - b;
            rg_sum += rg;
            rg_sq_sum += rg * rg;
            yb_sum += yb;
            yb_sq_sum += yb * yb;
        }
    }

    if brightness.is_empty() {
        brightness.push(0.0);
        saturation.push(0.0);
    }

    brightness.sort_by(f64::total_cmp);
    saturation.sort_by(f64::total_cmp);
    let sample_count = brightness.len() as f64;
    let brightness_mean = mean(&brightness);
    let brightness_median = percentile(&brightness, 0.50);
    let brightness_low_percentile = percentile(&brightness, 0.10);
    let brightness_high_percentile = percentile(&brightness, 0.90);
    let contrast = standard_deviation(&brightness, brightness_mean);
    let saturation_mean = mean(&saturation);
    let saturation_median = percentile(&saturation, 0.50);
    let shadow_ratio =
        brightness.iter().filter(|value| **value <= 0.15).count() as f64 / sample_count;
    let highlight_ratio =
        brightness.iter().filter(|value| **value >= 0.90).count() as f64 / sample_count;

    let mut ranked_colors: Vec<(f64, u8, u8, u8)> = weighted_color_bins
        .values()
        .map(|(weight, red, green, blue)| {
            let divisor = (*weight).max(f64::EPSILON);
            (
                *weight,
                (red / divisor).round().clamp(0.0, 255.0) as u8,
                (green / divisor).round().clamp(0.0, 255.0) as u8,
                (blue / divisor).round().clamp(0.0, 255.0) as u8,
            )
        })
        .collect();
    if ranked_colors.is_empty() {
        ranked_colors = color_bins
            .values()
            .map(|(count, red, green, blue)| {
                let divisor = (*count).max(1);
                (
                    0.0,
                    (red / divisor) as u8,
                    (green / divisor) as u8,
                    (blue / divisor) as u8,
                )
            })
            .collect();
    }
    ranked_colors.sort_by(|left, right| right.0.total_cmp(&left.0));

    let (dominant_r, dominant_g, dominant_b) = ranked_colors
        .first()
        .map(|(_, r, g, b)| (*r, *g, *b))
        .unwrap_or((0, 0, 0));
    let dominant_color_rgb = format!("#{dominant_r:02X}{dominant_g:02X}{dominant_b:02X}");
    let dominant_color_category = if chromatic_weight_total / sample_count >= 0.06 {
        color_category(dominant_r, dominant_g, dominant_b).to_owned()
    } else {
        neutral_category(brightness_mean, saturation_mean).to_owned()
    };
    let top_colors: Vec<serde_json::Value> = ranked_colors
        .iter()
        .take(5)
        .map(|(count, red, green, blue)| {
            serde_json::json!({
                "color": format!("#{red:02X}{green:02X}{blue:02X}"),
                "ratio": *count / sample_count,
            })
        })
        .collect();

    let rg_mean = rg_sum / sample_count;
    let yb_mean = yb_sum / sample_count;
    let rg_std = (rg_sq_sum / sample_count - rg_mean * rg_mean)
        .max(0.0)
        .sqrt();
    let yb_std = (yb_sq_sum / sample_count - yb_mean * yb_mean)
        .max(0.0)
        .sqrt();
    let colorfulness = (rg_std * rg_std + yb_std * yb_std).sqrt()
        + 0.3 * (rg_mean * rg_mean + yb_mean * yb_mean).sqrt();
    let warmth_score = if warm_weight > 0.0 {
        (warm_sum / warm_weight).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let neutral_ratio = (neutral_count as f64 / sample_count).clamp(0.0, 1.0);
    let monochrome_probability = (1.0 - saturation_mean * 2.5).clamp(0.0, 1.0);

    BasicImageFeatures {
        brightness_mean,
        brightness_median,
        brightness_low_percentile,
        brightness_high_percentile,
        shadow_ratio,
        highlight_ratio,
        contrast,
        dynamic_range: (brightness_high_percentile - brightness_low_percentile).max(0.0),
        tone_label: if brightness_mean < 0.36 {
            "low_key"
        } else if brightness_mean > 0.68 {
            "high_key"
        } else {
            "mid_tone"
        }
        .into(),
        exposure_label: if brightness_mean < 0.30 {
            "dark"
        } else if brightness_mean > 0.74 {
            "bright"
        } else {
            "normal"
        }
        .into(),
        contrast_label: if contrast < 0.11 {
            "low"
        } else if contrast > 0.24 {
            "high"
        } else {
            "medium"
        }
        .into(),
        saturation_mean,
        saturation_median,
        chroma_mean: (chroma_sum / sample_count).clamp(0.0, 1.0),
        dominant_color_rgb,
        dominant_color_category,
        dominant_colors_json: serde_json::to_string(&top_colors).unwrap_or_else(|_| "[]".into()),
        hue_histogram_json: serde_json::to_string(&hue_histogram).unwrap_or_else(|_| "[]".into()),
        warmth_score,
        neutral_ratio,
        colorfulness: colorfulness.clamp(0.0, 2.0),
        monochrome_probability,
        dominant_color_coverage: (chromatic_weight_total / sample_count).clamp(0.0, 1.0),
        saturation_label: if saturation_mean < 0.16 {
            "low"
        } else if saturation_mean > 0.52 {
            "high"
        } else {
            "medium"
        }
        .into(),
        algorithm_version: ANALYSIS_VERSION.into(),
    }
}

fn analysis_bounds(image: &RgbaImage) -> (u32, u32, u32, u32) {
    let (width, height) = image.dimensions();
    if width < 8 || height < 8 {
        return (0, 0, width, height);
    }

    let max_x_trim = width / 8;
    let max_y_trim = height / 8;
    let mut left = 0;
    let mut right = width;
    let mut top = 0;
    let mut bottom = height;

    while top < max_y_trim && border_row(image, top, left, right) {
        top += 1;
    }
    while bottom > height - max_y_trim
        && bottom > top + 1
        && border_row(image, bottom - 1, left, right)
    {
        bottom -= 1;
    }
    while left < max_x_trim && border_column(image, left, top, bottom) {
        left += 1;
    }
    while right > width - max_x_trim
        && right > left + 1
        && border_column(image, right - 1, top, bottom)
    {
        right -= 1;
    }

    (left, top, right, bottom)
}

fn border_row(image: &RgbaImage, y: u32, left: u32, right: u32) -> bool {
    let count = right.saturating_sub(left).max(1);
    let border = (left..right)
        .filter(|x| is_border_pixel(image.get_pixel(*x, y).0))
        .count() as u32;
    border as f64 / f64::from(count) >= 0.96
}

fn border_column(image: &RgbaImage, x: u32, top: u32, bottom: u32) -> bool {
    let count = bottom.saturating_sub(top).max(1);
    let border = (top..bottom)
        .filter(|y| is_border_pixel(image.get_pixel(x, *y).0))
        .count() as u32;
    border as f64 / f64::from(count) >= 0.96
}

fn is_border_pixel(pixel: [u8; 4]) -> bool {
    if pixel[3] < 16 {
        return true;
    }
    let r = f64::from(pixel[0]) / 255.0;
    let g = f64::from(pixel[1]) / 255.0;
    let b = f64::from(pixel[2]) / 255.0;
    let light = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let (saturation, _) = saturation_and_hue(r, g, b);
    (light <= 0.025 || light >= 0.975) && saturation <= 0.08
}

fn saturation_and_hue(r: f64, g: f64, b: f64) -> (f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let saturation = if max <= f64::EPSILON {
        0.0
    } else {
        delta / max
    };
    let hue = if delta <= f64::EPSILON {
        0.0
    } else if (max - r).abs() <= f64::EPSILON {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if (max - g).abs() <= f64::EPSILON {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    (saturation.clamp(0.0, 1.0), hue.rem_euclid(360.0))
}

fn color_category(red: u8, green: u8, blue: u8) -> &'static str {
    let r = f64::from(red) / 255.0;
    let g = f64::from(green) / 255.0;
    let b = f64::from(blue) / 255.0;
    let brightness = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let (saturation, hue) = saturation_and_hue(r, g, b);
    if saturation < 0.14 {
        return if brightness < 0.14 {
            "black"
        } else if brightness > 0.88 {
            "white"
        } else {
            "gray"
        };
    }
    match hue {
        value if !(15.0..345.0).contains(&value) => "red",
        value if value < 45.0 => "orange",
        value if value < 70.0 => "yellow",
        value if value < 165.0 => "green",
        value if value < 195.0 => "cyan",
        value if value < 255.0 => "blue",
        value if value < 315.0 => "purple",
        _ => "red",
    }
}

fn neutral_category(brightness: f64, saturation: f64) -> &'static str {
    let _ = (brightness, saturation);
    "neutral"
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len().max(1) as f64
}

fn percentile(values: &[f64], quantile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let index = ((values.len() - 1) as f64 * quantile).round() as usize;
    values[index.min(values.len() - 1)]
}

fn standard_deviation(values: &[f64], mean: f64) -> f64 {
    (values
        .iter()
        .map(|value| {
            let distance = value - mean;
            distance * distance
        })
        .sum::<f64>()
        / values.len().max(1) as f64)
        .sqrt()
}

#[derive(Debug, Default)]
struct SourceMetadata {
    exif: ExifMetadata,
    embedded_thumbnail: Option<Vec<u8>>,
}

fn read_exif(path: &Path) -> ExifMetadata {
    read_source_metadata(path).exif
}

fn read_source_metadata(path: &Path) -> SourceMetadata {
    let Ok(file) = File::open(path) else {
        return SourceMetadata {
            exif: ExifMetadata {
                orientation: 1,
                ..ExifMetadata::default()
            },
            embedded_thumbnail: None,
        };
    };
    read_exif_reader(&mut BufReader::new(file))
}

fn read_source_metadata_bytes(bytes: &[u8]) -> SourceMetadata {
    read_exif_reader(&mut Cursor::new(bytes))
}

fn read_exif_reader<R: BufRead + Seek>(reader: &mut R) -> SourceMetadata {
    let Ok(exif) = exif::Reader::new().read_from_container(reader) else {
        return SourceMetadata {
            exif: ExifMetadata {
                orientation: 1,
                ..ExifMetadata::default()
            },
            embedded_thumbnail: None,
        };
    };

    let field = |tag| exif.get_field(tag, In::PRIMARY);
    let capture_time = field(Tag::DateTimeOriginal)
        .or_else(|| field(Tag::DateTime))
        .and_then(ascii_value)
        .map(normalize_exif_datetime);
    let exposure_time = field(Tag::ExposureTime).map(|value| value.display_value().to_string());

    SourceMetadata {
        exif: ExifMetadata {
            orientation: field(Tag::Orientation)
                .and_then(|value| value.value.get_uint(0))
                .unwrap_or(1),
            capture_time,
            camera_make: field(Tag::Make).and_then(ascii_value),
            camera_model: field(Tag::Model).and_then(ascii_value),
            lens_model: field(Tag::LensModel).and_then(ascii_value),
            exposure_time,
            aperture: field(Tag::FNumber).and_then(rational_value),
            iso: field(Tag::PhotographicSensitivity)
                .and_then(|value| value.value.get_uint(0))
                .map(i64::from),
            focal_length: field(Tag::FocalLength).and_then(rational_value),
        },
        embedded_thumbnail: embedded_thumbnail(&exif),
    }
}

fn embedded_thumbnail(exif: &exif::Exif) -> Option<Vec<u8>> {
    let offset = exif
        .get_field(Tag::JPEGInterchangeFormat, In::THUMBNAIL)
        .and_then(|field| field.value.get_uint(0))? as usize;
    let length = exif
        .get_field(Tag::JPEGInterchangeFormatLength, In::THUMBNAIL)
        .and_then(|field| field.value.get_uint(0))? as usize;
    let bytes = exif.buf().get(offset..offset.checked_add(length)?)?;
    (bytes.starts_with(&[0xff, 0xd8]) && bytes.ends_with(&[0xff, 0xd9])).then(|| bytes.to_vec())
}

fn ascii_value(field: &exif::Field) -> Option<String> {
    let Value::Ascii(values) = &field.value else {
        return None;
    };
    let value = values.first()?;
    let decoded = String::from_utf8_lossy(value)
        .trim_matches(char::from(0))
        .trim()
        .to_owned();
    (!decoded.is_empty()).then_some(decoded)
}

fn rational_value(field: &exif::Field) -> Option<f64> {
    match &field.value {
        Value::Rational(values) => values.first().and_then(|value| {
            (value.denom != 0).then_some(f64::from(value.num) / f64::from(value.denom))
        }),
        Value::SRational(values) => values.first().and_then(|value| {
            (value.denom != 0).then_some(f64::from(value.num) / f64::from(value.denom))
        }),
        _ => None,
    }
}

fn normalize_exif_datetime(value: String) -> String {
    if value.len() >= 19 {
        let mut bytes = value.into_bytes();
        if bytes.get(4) == Some(&b':') && bytes.get(7) == Some(&b':') {
            bytes[4] = b'-';
            bytes[7] = b'-';
        }
        if bytes.get(10) == Some(&b' ') {
            bytes[10] = b'T';
        }
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        value
    }
}

fn apply_orientation(image: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate270().fliph(),
        8 => image.rotate270(),
        _ => image,
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn thumbnail_path_for_key(thumbnail_dir: &Path, cache_key: &str) -> PathBuf {
    thumbnail_dir.join(format!("{cache_key}-{THUMBNAIL_SPEC}.jpg"))
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::*;

    fn assert_in_unit_range(value: f64) {
        assert!((0.0..=1.0).contains(&value), "value out of range: {value}");
    }

    #[test]
    fn black_white_gray_and_saturated_images_have_expected_ranges() {
        for pixel in [
            Rgba([0, 0, 0, 255]),
            Rgba([255, 255, 255, 255]),
            Rgba([128, 128, 128, 255]),
            Rgba([255, 0, 0, 255]),
        ] {
            let image = RgbaImage::from_pixel(16, 16, pixel);
            let features = analyze_rgba(&image);
            assert_in_unit_range(features.brightness_mean);
            assert_in_unit_range(features.saturation_mean);
            assert_in_unit_range(features.shadow_ratio);
            assert_in_unit_range(features.highlight_ratio);
            assert_in_unit_range(features.neutral_ratio);
            assert_in_unit_range(features.monochrome_probability);
        }
    }

    #[test]
    fn transparent_pixels_are_ignored() {
        let mut image = RgbaImage::from_pixel(2, 1, Rgba([255, 255, 255, 0]));
        image.put_pixel(1, 0, Rgba([255, 0, 0, 255]));
        let features = analyze_rgba(&image);
        assert!(features.saturation_mean > 0.99);
        assert_eq!(features.dominant_color_category, "red");
    }

    #[test]
    fn chromatic_pixels_beat_large_dark_silhouette_and_gray_stays_neutral() {
        let mut sunset = RgbaImage::from_pixel(32, 32, Rgba([4, 5, 8, 255]));
        for y in 20..32 {
            for x in 0..32 {
                sunset.put_pixel(x, y, Rgba([230, 55, 18, 255]));
            }
        }
        let sunset_features = analyze_rgba(&sunset);
        assert_eq!(sunset_features.dominant_color_category, "red");
        assert!(sunset_features.dominant_color_coverage > 0.06);

        let gray = RgbaImage::from_pixel(32, 32, Rgba([124, 126, 128, 255]));
        let gray_features = analyze_rgba(&gray);
        assert_eq!(gray_features.dominant_color_category, "neutral");
        assert!(gray_features.neutral_ratio > 0.9);
    }

    #[test]
    fn tiny_images_are_supported() {
        let image = RgbaImage::from_pixel(1, 1, Rgba([20, 80, 200, 255]));
        let features = analyze_rgba(&image);
        assert_in_unit_range(features.brightness_mean);
        assert_in_unit_range(features.saturation_mean);
    }

    #[test]
    fn orientation_rotates_dimensions() {
        let image = DynamicImage::ImageRgba8(RgbaImage::new(3, 2));
        let rotated = apply_orientation(image, 6);
        assert_eq!(rotated.dimensions(), (2, 3));
    }

    #[test]
    fn thumbnail_cache_is_outside_source_and_does_not_modify_source() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_dir = temp.path().join("图库 😀");
        let cache_dir = temp.path().join("app-data").join("thumbs");
        fs::create_dir_all(&source_dir).expect("source dir");
        let source = source_dir.join("красный image.png");
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 255])))
            .save(&source)
            .expect("save fixture");
        let before = fs::read(&source).expect("read source");

        let first = process_image(&source, &cache_dir, "same-fingerprint").expect("process image");
        let first_thumb = fs::read(&first.thumbnail_path).expect("read thumbnail");
        let second = process_image(&source, &cache_dir, "same-fingerprint").expect("cache hit");
        let source_bytes = fs::read(&source).expect("source bytes");
        let from_bytes = process_image_with_source_bytes(
            &source,
            &cache_dir,
            "bytes-fingerprint",
            Some(&source_bytes),
        )
        .expect("process from source bytes");
        let cached_dimensions = image::ImageReader::open(&first.thumbnail_path)
            .expect("open thumbnail")
            .decode()
            .expect("decode thumbnail")
            .dimensions();

        assert_eq!(fs::read(&source).expect("source after"), before);
        assert_eq!(first.thumbnail_path, second.thumbnail_path);
        assert_eq!(
            fs::read(&second.thumbnail_path).expect("second thumb"),
            first_thumb
        );
        assert_eq!(first.features, from_bytes.features);
        assert_eq!(first.features.algorithm_version, ANALYSIS_VERSION);
        assert_eq!(cached_dimensions, (8, 8));
        assert!(
            first
                .thumbnail_path
                .ends_with(&format!("{THUMBNAIL_SPEC}.jpg"))
        );
        assert!(Path::new(&first.thumbnail_path).starts_with(&cache_dir));
        assert!(!source_dir.join("thumbnails").exists());
    }

    #[test]
    fn thumbnail_cache_uses_one_bounded_resize_for_wide_image() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("wide.png");
        let cache_dir = temp.path().join("cache");
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(1200, 600, Rgba([30, 120, 210, 255])))
            .save(&source)
            .expect("save wide fixture");

        let processed =
            process_image(&source, &cache_dir, "wide-fingerprint").expect("process wide fixture");
        let dimensions = image::ImageReader::open(&processed.thumbnail_path)
            .expect("open wide thumbnail")
            .decode()
            .expect("decode wide thumbnail")
            .dimensions();

        assert_eq!(dimensions, (640, 320));
        assert_eq!(processed.features.algorithm_version, ANALYSIS_VERSION);
    }

    #[test]
    fn cached_thumbnail_reanalysis_never_decodes_source_pixels() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("source.png");
        let cache_dir = temp.path().join("cache");
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(1600, 900, Rgba([30, 120, 210, 255])))
            .save(&source)
            .expect("save source");

        let first = process_image(&source, &cache_dir, "cache-reuse").expect("cold process");
        let reused = process_image_from_cached_thumbnail(&source, Path::new(&first.thumbnail_path))
            .expect("cached process");

        assert_eq!(reused.width, 1600);
        assert_eq!(reused.height, 900);
        assert_eq!(reused.timings.source_decode_us, 0);
        assert!(reused.timings.thumbnail_decode_us > 0);
        assert!(reused.timings.source_dimension_us > 0);
        assert_eq!(reused.timings.resize_us, 0);
        assert_eq!(reused.features.algorithm_version, ANALYSIS_VERSION);
    }

    #[test]
    fn embedded_jpeg_thumbnail_avoids_primary_pixel_decode() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("embedded.jpg");
        let cache_dir = temp.path().join("cache");
        let primary = RgbaImage::from_pixel(1600, 900, Rgba([20, 80, 200, 255]));
        let embedded = RgbaImage::from_pixel(320, 180, Rgba([220, 40, 30, 255]));
        let source_bytes = jpeg_with_embedded_thumbnail(&primary, &embedded);
        fs::write(&source, &source_bytes).expect("write source");

        let processed = process_image_with_source_bytes(
            &source,
            &cache_dir,
            "embedded-cache",
            Some(&source_bytes),
        )
        .expect("process embedded preview");
        let thumbnail_dimensions = image::ImageReader::open(&processed.thumbnail_path)
            .expect("open generated thumbnail")
            .decode()
            .expect("decode generated thumbnail")
            .dimensions();

        assert_eq!((processed.width, processed.height), (900, 1600));
        assert_eq!(thumbnail_dimensions, (180, 320));
        assert_eq!(processed.timings.source_decode_us, 0);
        assert!(processed.timings.thumbnail_decode_us > 0);
    }

    fn jpeg_with_embedded_thumbnail(primary: &RgbaImage, thumbnail: &RgbaImage) -> Vec<u8> {
        let mut primary_bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut primary_bytes, 92)
            .encode_image(primary)
            .expect("encode primary");
        let mut thumbnail_bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut thumbnail_bytes, 84)
            .encode_image(thumbnail)
            .expect("encode thumbnail");

        let thumbnail_offset = 56_u32;
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42_u16.to_le_bytes());
        tiff.extend_from_slice(&8_u32.to_le_bytes());
        tiff.extend_from_slice(&1_u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112_u16.to_le_bytes());
        tiff.extend_from_slice(&3_u16.to_le_bytes());
        tiff.extend_from_slice(&1_u32.to_le_bytes());
        tiff.extend_from_slice(&[6, 0, 0, 0]);
        tiff.extend_from_slice(&26_u32.to_le_bytes());
        tiff.extend_from_slice(&2_u16.to_le_bytes());
        tiff.extend_from_slice(&0x0201_u16.to_le_bytes());
        tiff.extend_from_slice(&4_u16.to_le_bytes());
        tiff.extend_from_slice(&1_u32.to_le_bytes());
        tiff.extend_from_slice(&thumbnail_offset.to_le_bytes());
        tiff.extend_from_slice(&0x0202_u16.to_le_bytes());
        tiff.extend_from_slice(&4_u16.to_le_bytes());
        tiff.extend_from_slice(&1_u32.to_le_bytes());
        tiff.extend_from_slice(&(thumbnail_bytes.len() as u32).to_le_bytes());
        tiff.extend_from_slice(&0_u32.to_le_bytes());
        assert_eq!(tiff.len(), thumbnail_offset as usize);
        tiff.extend_from_slice(&thumbnail_bytes);

        let app1_length = u16::try_from(2 + 6 + tiff.len()).expect("APP1 length");
        let mut output = vec![0xff, 0xd8, 0xff, 0xe1];
        output.extend_from_slice(&app1_length.to_be_bytes());
        output.extend_from_slice(b"Exif\0\0");
        output.extend_from_slice(&tiff);
        output.extend_from_slice(&primary_bytes[2..]);
        output
    }
}
