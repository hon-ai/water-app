#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod orchestrator_service;
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
        .plugin(tauri_plugin_shell::init())
        // Auto-updater. Pointed at the `latest.json` manifest that
        // `tauri-action` attaches to each GitHub Release. The JS
        // side calls `check()` in `App.tsx` once on boot; if the
        // pubkey in `tauri.conf.json` is the placeholder, the
        // renderer catches the resulting error silently.
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            commands::scene::scene_read_metadata,
            commands::scene::scene_set_location,
            commands::scene::scene_set_summary,
            commands::character::character_create,
            commands::character::character_read,
            commands::character::character_list,
            commands::character::character_update_field,
            commands::character::character_delete,
            commands::character::character_link_to_scene,
            commands::character::character_unlink_from_scene,
            commands::character::character_set_pov,
            commands::character::intake_schema,
            commands::character::character_autosuggest_for_scene,
            commands::provider::provider_test,
            commands::provider::provider_set_key,
            commands::provider::provider_set_model,
            commands::diagnostics::diagnostics_status,
            commands::events::bus_ping,
            commands::events::typing_telemetry,
            commands::events::scene_state,
            commands::pill::pill_expand,
            commands::pill::pill_regenerate,
            commands::pill::pill_pin,
            commands::pill::pill_dismiss,
            commands::pill::pill_evicted,
            commands::pill::feedback_reset,
            commands::pill::pill_deepen,
            commands::pill::rabbit_deepen_thought,
            commands::pill::rabbit_set_resonance,
            commands::pill::pinned_list,
            commands::editor_pill::editor_pills_run,
            commands::editor_pill::editor_pills_list,
            commands::editor_pill::editor_pill_dismiss,
            commands::editor_pill::editor_polish_request,
            commands::world::world_segment_list,
            commands::world::world_segment_create,
            commands::world::world_segment_update_template,
            commands::world::world_segment_set_hidden,
            commands::world::world_segment_delete,
            commands::world::world_intake_schema,
            commands::world::world_single_doc_read,
            commands::world::world_single_doc_update_field,
            commands::world::world_entry_list,
            commands::world::world_entry_read,
            commands::world::world_entry_create,
            commands::world::world_entry_update_field,
            commands::world::world_entry_update_aliases,
            commands::world::world_entry_delete_if_empty,
            commands::world::world_entry_delete,
            commands::world::world_autosuggest,
            commands::heat::heat_read,
            commands::heat::heat_set_metric_enabled,
            commands::heat::heat_read_settings,
            commands::canvas::scene_canvas_list,
            commands::canvas::scene_canvas_set_position,
            commands::canvas::scene_canvas_set_group,
            commands::canvas::scene_canvas_reset_all,
            commands::uv::check_uv_installed,
            commands::uv::install_uv,
            commands::uv::restart_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
