use serde::Serialize;
use tauri::State;
use crate::state::AppState;

#[derive(Serialize)]
pub struct SceneInfo {
    pub id: String,
    pub name: String,
    pub ordering: i64,
    pub word_count: i64,
}

#[tauri::command]
pub async fn scene_create(_state: State<'_, AppState>, _name: String) -> Result<SceneInfo, String> {
    Err("scene_create: implemented in Task 37".into())
}
#[tauri::command]
pub async fn scene_read(_state: State<'_, AppState>, _id: String) -> Result<String, String> {
    Err("scene_read: implemented in Task 37".into())
}
#[tauri::command]
pub async fn scene_write_body(_state: State<'_, AppState>, _id: String, _body: String) -> Result<SceneInfo, String> {
    Err("scene_write_body: implemented in Task 37".into())
}
#[tauri::command]
pub async fn scene_list(_state: State<'_, AppState>) -> Result<Vec<SceneInfo>, String> {
    Err("scene_list: implemented in Task 37".into())
}
