# SigLIP 2 Base Patch16 224 INT8

This directory contains the locally bundled ONNX INT8 export used by the
default photographer-facing topic model.

- Upstream model family: [google/siglip2-base-patch16-224](https://huggingface.co/google/siglip2-base-patch16-224)
- ONNX export: [onnx-community/siglip2-base-patch16-224-ONNX](https://huggingface.co/onnx-community/siglip2-base-patch16-224-ONNX)
- License: Apache-2.0 (see the upstream Google model card)
- Downloaded: 2026-08-11

Verified files:

| File              | SHA-256                                                            |
| ----------------- | ------------------------------------------------------------------ |
| `model_int8.onnx` | `bfe28fe2ccdb685874586648035ea349593e487ce33bd0939b28813681a8f167` |
| `tokenizer.json`  | `cb9140fae3ac5122c972d37adf83e1248471a38147ad76f8215c8872c6fd8322` |
| `tokenizer.model` | `61a7b147390c64585d6c3543dd6fc636906c9af3865a5548f27f31aee1d4c8e2` |

The adapter uses the exported `input_ids` and `pixel_values` inputs, 64-token
padding, 224x224 resizing, and SigLIP's 0.5 mean/std normalization. It is
selected by default for photographer-facing topic recognition. TinyCLIP and
MobileCLIP are historical alternatives and are not bundled or exposed by the
current MVP.
