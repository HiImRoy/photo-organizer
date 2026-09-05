# ONNX Runtime resource

- Runtime: Microsoft ONNX Runtime 1.24.1 Windows x64 DirectML package
- Source package: <https://www.nuget.org/packages/Microsoft.ML.OnnxRuntime.DirectML/1.24.1>
- Upstream runtime release: <https://github.com/microsoft/onnxruntime/releases/tag/v1.24.1>
- Downloaded: 2026-08-25
- License: MIT (ONNXRUNTIME-LICENSE.txt)
- Main runtime DLL: onnxruntime.dll
- Main runtime DLL SHA-256: 03bd80df147fcbbec55fea8bc6f7bdd02b84878992adb04f6ddca03cd5342fad
- Shared provider DLL: onnxruntime_providers_shared.dll
- Shared provider DLL SHA-256: 534b5ebdd6d78aae96b33cfc9b44444e4b424645417f0b58faebae2c004d5982

The DirectML package includes the CPU execution provider as the fallback provider. PhotoOrganizer registers DirectML only after Windows hardware detection and runtime/provider probing; a model session failure falls back to CPU and is reported in the runtime status. CUDA and cuDNN are not bundled.

ONNXRUNTIME-THIRD-PARTY-NOTICES.txt is preserved alongside the runtime. The system DirectML component is supplied by Windows and is not redistributed by this application.
