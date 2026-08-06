# Third-party notices

PhotoOrganizer uses open-source packages recorded by `package-lock.json` and `src-tauri/Cargo.lock`. The authoritative license texts remain with each dependency and must be collected into release artifacts before public distribution.

The main runtime building blocks are React, Tauri, rusqlite/libsqlite3, image, kamadak-exif, serde, tokio, and their locked transitive dependencies. Their package metadata and license expressions are the source for the release inventory; this repository does not relicense those components. The bundled WebView2 offline installer and Tauri NSIS tooling remain subject to their upstream redistribution terms.

The semantic workspace release bundles the following additional MIT-licensed artifacts:

- TinyCLIP code/model lineage: Microsoft Cream/TinyCLIP and `wkcn/TinyCLIP-ViT-8M-16-Text-3M-YFCC15M`.
- ONNX conversion: `onnx-community/TinyCLIP-ViT-8M-16-Text-3M-YFCC15M-ONNX`, including `model-int8.onnx`, tokenizer and preprocessing metadata.
- Microsoft ONNX Runtime 1.24.1 Windows x64 CPU binary (`onnxruntime.dll`).

Exact source URLs, hashes and upstream license/notice files are archived under `src-tauri/resources/models/tinyclip-vit-8m-16-text-3m-yfcc15m/` and `src-tauri/resources/runtime/`. The release does **not** include Python, OpenCV, ExifTool, CUDA or cuDNN.

This file is a release checklist, not a substitute for the license metadata generated from locked dependencies. Before a public release:

1. Generate JavaScript and Rust dependency license reports from the lockfiles.
2. Review reciprocal, attribution, binary redistribution, patent, and notice obligations.
3. Archive exact source/version/license URLs and hashes for native binaries.
4. Review model code and model weights separately.
5. Include required notices and full license texts in the installer.
