//! Pill verbs invoked from the renderer. M2 ships stubs that emit
//! placeholder events; full wiring (orchestrator -> router -> prompts -> LLM
//! -> bouquet:ready) lands in Phase F Task 26.

use crate::events::emit;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize, Clone)]
struct BouquetReady {
    parent_pill_id: String,
    items: Vec<BouquetItem>,
}

#[derive(Serialize, Clone)]
struct BouquetItem {
    sub_pill_id: String,
    angle: String,
    text: String,
}

#[tauri::command]
pub async fn pill_expand(app: AppHandle, parent_pill_id: String) -> Result<(), String> {
    let payload = BouquetReady {
        parent_pill_id: parent_pill_id.clone(),
        items: vec![
            BouquetItem {
                sub_pill_id: format!("{parent_pill_id}-1"),
                angle: "feel".into(),
                text: "(stub) feel something at the threshold".into(),
            },
            BouquetItem {
                sub_pill_id: format!("{parent_pill_id}-2"),
                angle: "notice".into(),
                text: "(stub) the bell rings somewhere unseen".into(),
            },
            BouquetItem {
                sub_pill_id: format!("{parent_pill_id}-3"),
                angle: "wonder".into(),
                text: "(stub) what is held in that pause".into(),
            },
        ],
    };
    emit(&app, "bouquet:ready", payload).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pill_regenerate(app: AppHandle, parent_pill_id: String) -> Result<(), String> {
    pill_expand(app, parent_pill_id).await
}

#[tauri::command]
pub async fn pill_pin(_pill_id: String) -> Result<(), String> {
    // Phase F writes to pinned_pill table.
    Ok(())
}

#[tauri::command]
pub async fn pill_dismiss(app: AppHandle, pill_id: String) -> Result<(), String> {
    #[derive(Serialize, Clone)]
    struct Dismiss {
        pill_id: String,
    }
    emit(&app, "pill:dismissed", Dismiss { pill_id }).map_err(|e| e.to_string())
}
