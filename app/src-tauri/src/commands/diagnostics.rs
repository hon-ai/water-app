use serde::Serialize;
use tauri::State;
use crate::state::AppState;

#[derive(Serialize)]
pub struct DiagnosticsStatus {
    pub has_open_project: bool,
    pub project_root: Option<String>,
    pub providers: Vec<String>,
}

#[tauri::command]
pub async fn diagnostics_status(state: State<'_, AppState>) -> Result<DiagnosticsStatus, String> {
    let proj = state.project.lock().await;
    let has = proj.is_some();
    let root = proj.as_ref().map(|p| p.root.to_string_lossy().to_string());
    Ok(DiagnosticsStatus {
        has_open_project: has,
        project_root: root,
        providers: vec![],
    })
}
