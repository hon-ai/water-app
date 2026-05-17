use crate::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct DiagnosticsStatus {
    pub has_open_project: bool,
    pub project_root: Option<String>,
    pub providers: Vec<String>,
    pub router_configured: bool,
}

#[tauri::command]
pub async fn diagnostics_status(state: State<'_, AppState>) -> Result<DiagnosticsStatus, String> {
    let proj = state.project.lock().await;
    let router = state.router.lock().await;
    Ok(DiagnosticsStatus {
        has_open_project: proj.is_some(),
        project_root: proj.as_ref().map(|p| p.root.to_string_lossy().to_string()),
        providers: vec![
            "canned".into(),
            "anthropic".into(),
            "openai".into(),
            "ollama".into(),
            "llamacpp".into(),
        ],
        router_configured: router.is_some(),
    })
}
