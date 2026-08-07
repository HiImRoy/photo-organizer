pub mod db;
pub mod error;
pub mod imaging;
#[cfg(feature = "desktop")]
pub mod ipc;
pub mod models;
pub mod organization;
pub mod paths;
pub mod scanner;
pub mod semantic;
pub mod semantic_tasks;
pub mod tasks;

#[cfg(feature = "desktop")]
use tauri::Manager;

#[cfg(feature = "desktop")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let resource_dir = app.path().resource_dir()?;
            let paths = paths::AppPaths::initialize_with_resources(data_dir, resource_dir)?;
            let repository = db::Repository::new(&paths.database_path);
            repository.initialize()?;
            let semantic: std::sync::Arc<dyn semantic::SemanticClassifier> =
                match semantic::TinyClipClassifier::load(
                    &paths.semantic_model_dir,
                    &paths.onnx_runtime_path,
                ) {
                    Ok(classifier) => {
                        repository.register_semantic_model(
                            &paths.semantic_model_dir.join(semantic::MODEL_FILE),
                            &paths.semantic_model_dir.join(semantic::TOKENIZER_FILE),
                        )?;
                        std::sync::Arc::new(classifier)
                    }
                    Err(error) => {
                        log::warn!("semantic model unavailable: {error}");
                        std::sync::Arc::new(semantic::UnavailableClassifier::with_message(
                            error.to_string(),
                        ))
                    }
                };
            log::info!(
                "PhotoOrganizer data directory: {}",
                paths.data_dir.display()
            );
            let state = ipc::AppState::new(repository, paths, semantic);
            ipc::resume_pending_semantic_jobs(app.handle().clone(), &state);
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::list_libraries,
            ipc::list_assets,
            ipc::start_scan,
            ipc::cancel_scan,
            ipc::get_thumbnail_data_url,
            ipc::get_preview_data_url,
            ipc::remove_library,
            ipc::open_library_in_explorer,
            ipc::get_semantic_status,
            ipc::prepare_semantic_model,
            ipc::get_semantic_catalog,
            ipc::list_library_folders,
            ipc::list_semantic_groups,
            ipc::get_semantic_progress,
            ipc::start_semantic_analysis,
            ipc::start_semantic_analysis_selected,
            ipc::reanalyze_asset,
            ipc::pause_semantic_analysis,
            ipc::resume_semantic_analysis,
            ipc::cancel_semantic_analysis,
            ipc::validate_organization_rules,
            ipc::preview_organization_plan,
            ipc::get_organization_plan,
            ipc::list_organization_issues,
            ipc::export_organization_manifest,
            ipc::discard_organization_plan,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run PhotoOrganizer");
}
