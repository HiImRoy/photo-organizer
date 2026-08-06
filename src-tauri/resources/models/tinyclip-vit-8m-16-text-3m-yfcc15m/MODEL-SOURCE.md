# TinyCLIP model resource

- Model: `TinyCLIP-ViT-8M-16-Text-3M-YFCC15M`
- Analysis version: `photo-organizer-semantic-v1`
- Format: full INT8 ONNX graph
- Model source: <https://huggingface.co/onnx-community/TinyCLIP-ViT-8M-16-Text-3M-YFCC15M-ONNX>
- Original model source: <https://huggingface.co/wkcn/TinyCLIP-ViT-8M-16-Text-3M-YFCC15M>
- Original code: <https://github.com/microsoft/Cream/tree/main/TinyCLIP>
- Downloaded: 2026-08-07
- Code and weight license: MIT (copies in `MIT-LICENSE.txt`)

## Integrity

| File                       | SHA-256                                                            |
| -------------------------- | ------------------------------------------------------------------ |
| `model-int8.onnx`          | `10921310ddef06557ec1598d1260470a0a4db53f70ffe0deb60b946dcad6d27a` |
| `tokenizer.json`           | `6d9109cc838977f3ca94a379eec36aecc7c807e1785cd729660ca2fc0171fb35` |
| `preprocessor_config.json` | `5df7e578c37e907a431daf47fd592fc49fa50d23ed4c41285a0a34a58a9d2e06` |
| `config.json`              | `0ca46b868f12305e959a1cfa2b8085e7bffd521f68769ec3bf2999986b55bec3` |
| `MIT-LICENSE.txt`          | `4ecaa5a5bc48d91c899216ba3ab8d9fccbb543e106b75f5792e455631d599ae5` |

The application verifies the model and tokenizer hashes before enabling inference.
Similarity values are raw image/text embedding similarities. They are not probabilities or accuracy estimates.
