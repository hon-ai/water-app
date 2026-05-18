#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod state;

use state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::project::create_project,
            commands::project::open_project,
            commands::project::close_project,
            commands::scene::scene_create,
            commands::scene::scene_read,
            commands::scene::scene_write_body,
            commands::scene::scene_list,
            commands::scene::scene_rename,
            commands::provider::provider_test,
            commands::provider::provider_set_key,
            commands::diagnostics::diagnostics_status,
            commands::events::bus_ping,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
