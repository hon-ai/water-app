use tauri::State;
use crate::state::AppState;

#[tauri::command]
pub async fn provider_test(_state: State<'_, AppState>, _provider_id: String) -> Result<Vec<String>, String> {
    Err("provider_test: implemented in Task 37".into())
}
#[tauri::command]
pub async fn provider_set_key(_state: State<'_, AppState>, _provider_id: String, _key: String) -> Result<(), String> {
    Err("provider_set_key: implemented in Task 37".into())
}
