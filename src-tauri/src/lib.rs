pub mod classification;
pub mod db;
pub mod error;
pub mod imaging;
#[cfg(feature = "desktop")]
pub mod ipc;
pub mod models;
pub mod organization;
pub mod paths;
pub mod places365;
pub mod scanner;
pub mod semantic;
pub mod semantic_tasks;
pub mod source_identity;
pub mod subject;
pub mod tasks;
pub mod topics;
#[cfg(windows)]
mod wic_thumbnail;
pub mod workflow;

#[cfg(feature = "desktop")]
use tauri::Manager;

#[cfg(feature = "desktop")]
use std::path::PathBuf;

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
            let data_dir = std::env::var_os("PHOTO_ORGANIZER_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?);
            let resource_dir = app.path().resource_dir()?;
            let paths = paths::AppPaths::initialize_with_resources(data_dir, resource_dir)?;
            let repository = db::Repository::new(&paths.database_path);
            repository.initialize()?;
            // Keep first-run startup light: model sessions are loaded after the
            // state is installed and the WebView can be created. Once a model
            // has been explicitly prepared, ipc::restore_persisted_models
            // restores it in the background on later launches.
            let semantic: std::sync::Arc<dyn semantic::SemanticClassifier> =
                std::sync::Arc::new(semantic::UnavailableClassifier::default());
            let subject: std::sync::Arc<dyn subject::SubjectClassifier> =
                std::sync::Arc::new(subject::UnavailableSubjectClassifier::default());
            log::info!(
                "PhotoOrganizer data directory: {}",
                paths.data_dir.display()
            );
            let state = ipc::AppState::new(repository, paths, semantic, subject);
            app.manage(state);
            // The manual acceptance config sets `create: false` so the script
            // can provide an isolated, per-session WebView2 profile. The
            // packaged/default config still creates its `main` window before
            // setup and therefore takes this branch only when it is absent.
            if app.get_webview_window("main").is_none() {
                let window_config = app
                    .config()
                    .app
                    .windows
                    .iter()
                    .find(|window| window.label == "main")
                    .ok_or(tauri::Error::WindowNotFound)?;
                let mut window_builder =
                    tauri::WebviewWindowBuilder::from_config(app.handle(), window_config)?;
                if let Some(data_directory) = std::env::var_os("PHOTO_ORGANIZER_WEBVIEW_DATA_DIR") {
                    window_builder = window_builder.data_directory(PathBuf::from(data_directory));
                }
                window_builder.build()?;
            }
            ipc::restore_persisted_models(
                app.handle().clone(),
                app.state::<ipc::AppState>().inner(),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::list_libraries,
            ipc::list_assets,
            ipc::query_assets,
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
            ipc::get_subject_status,
            ipc::prepare_subject_model,
            ipc::clear_subject_data,
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
            ipc::list_browse_nodes,
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
