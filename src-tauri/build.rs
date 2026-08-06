fn main() {
    #[cfg(target_env = "msvc")]
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        tauri_build::build();
    }
}
