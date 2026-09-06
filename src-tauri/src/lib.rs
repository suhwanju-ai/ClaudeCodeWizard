pub mod cli_check;
pub mod commands;
pub mod engine;
pub mod template;

use engine::executor::ExecutorConfig;
use engine::orchestrator::Orchestrator;
use engine::run_record::RunRecordStore;
use tauri::Manager;
use template::store::TemplateStore;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("failed to resolve app data dir");
            let orchestrator = Orchestrator::new(
                TemplateStore::new(app_data_dir.join("templates")),
                RunRecordStore::new(app_data_dir.join("runs")),
                ExecutorConfig::default(),
            );
            if let Err(e) = template::seed::seed_default_templates(&orchestrator.template_store) {
                eprintln!("failed to seed default templates: {e}");
            }
            app.manage(orchestrator);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_templates,
            commands::load_template,
            commands::save_template,
            commands::delete_template,
            commands::check_cli,
            commands::start_pipeline_run,
            commands::approve_checkpoint,
            commands::request_changes,
            commands::reject_checkpoint,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
