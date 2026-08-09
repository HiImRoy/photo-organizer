pub mod classification;
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
pub mod source_identity;
pub mod tasks;
pub mod workflow;

#[cfg(feature = "desktop")]
use tauri::Manager;

#[cfg(feature = "desktop")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
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
            ipc::get_classification_registry,
            ipc::get_asset_detail,
            ipc::update_classification_override,
            ipc::update_asset_rating,
            ipc::update_asset_color_label,
            ipc::update_tag_override,
            ipc::restore_auto_classification,
            ipc::batch_update_classification,
            ipc::set_library_parent,
            ipc::assign_asset_to_library,
            ipc::start_scan,
            ipc::rescan_library,
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
            ipc::list_favorite_asset_ids,
            ipc::list_favorite_assets,
            ipc::set_asset_favorite,
            ipc::list_collections,
            ipc::create_collection,
            ipc::delete_collection,
            ipc::get_collection,
            ipc::add_assets_to_collection,
            ipc::remove_assets_from_collection,
            ipc::list_duplicate_groups,
            ipc::search_local_images,
            ipc::find_similar_assets,
            ipc::build_similarity_clusters,
            ipc::get_face_feature_status,
            ipc::clear_face_data,
            ipc::render_edit_preview,
            ipc::preview_edit_export,
            ipc::execute_edit_export,
            ipc::preview_edit_rollback,
            ipc::execute_edit_rollback,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run PhotoOrganizer");
}
