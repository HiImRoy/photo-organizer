# Test data boundary

Only synthetic or redistributable fixtures may be placed here. Automated tests may read this directory and may copy inputs into a test-owned temporary directory, but must never write back to tracked fixtures.

The Rust integration tests generate small deterministic images inside temporary directories so Unicode path variants and source-file hash invariants can be tested without accessing a personal photo library.
