//! Pill verbs invoked from the renderer. M2 Phase E ships:
//! - `pill_expand` / `pill_regenerate`: stubbed bouquets (Phase F replaces).
//! - `pill_pin`: writes to `pinned_pill` + emits `pill:pinned`.
//! - `pill_dismiss`: deletes from `pinned_pill` (if present) + emits both
//!   `pill:dismissed` and `pill:unpinned`.
//! - `pinned_list`: read-side query used by the renderer's PinnedColumn on
//!   mount to rehydrate the existing pin set.

use crate::events::emit;
use serde::{Deserialize, Serialize};
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

/// Payload mirrored on the renderer side as `Pill` (see
/// `app/src/pill/types.ts`). Used as both the `pill_pin` input and the
/// `pinned_list` row shape, plus the `pill:pinned` event body.
#[derive(Serialize, Deserialize, Clone)]
pub struct PinnedPill {
    pub pill_id: String,
    pub speaker_id: String,
    pub hue_token: String,
    pub text: String,
    pub block_target_id: Option<String>,
    pub trigger_id: String,
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
pub async fn pill_pin(
    app: AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    pill: PinnedPill,
    scene_id: String,
    block_id: String,
    snippet: String,
) -> Result<(), String> {
    let db = {
        let proj = state.project.lock().await;
        proj.as_ref().ok_or("no project open")?.db.clone()
    };
    let g = db.lock().await;
    let now = chrono::Utc::now().to_rfc3339();
    // INSERT OR IGNORE: re-pinning the same pill_id is a no-op (the row
    // already records the original pin time + bouquet context). The
    // renderer should still get a `pill:pinned` event so PinnedColumn
    // reflects the user intent.
    g.conn()
        .execute(
            "INSERT OR IGNORE INTO pinned_pill \
             (id, scene_id, block_id, snippet, speaker_kind, speaker_id, message, hue, \
              rabbit_hole_path, created_at, parent_pill_id, pinned_at, trigger_class, bouquet_position) \
             VALUES (?1, ?2, ?3, ?4, 'persona', ?5, ?6, ?7, NULL, ?8, NULL, ?8, ?9, NULL)",
            rusqlite::params![
                pill.pill_id,
                scene_id,
                block_id,
                snippet,
                pill.speaker_id,
                pill.text,
                pill.hue_token,
                now,
                pill.trigger_id,
            ],
        )
        .map_err(|e| e.to_string())?;
    drop(g);
    emit(&app, "pill:pinned", pill).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pill_dismiss(
    app: AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    pill_id: String,
) -> Result<(), String> {
    // Delete from pinned_pill if present. "no project open" is not an
    // error here — un-pinning during shutdown should still emit events so
    // the renderer can clean up its state.
    let db_opt = {
        let proj = state.project.lock().await;
        proj.as_ref().map(|p| p.db.clone())
    };
    if let Some(db) = db_opt {
        let g = db.lock().await;
        let _ = g
            .conn()
            .execute(
                "DELETE FROM pinned_pill WHERE id = ?1",
                rusqlite::params![pill_id],
            );
    }
    #[derive(Serialize, Clone)]
    struct Dismiss {
        pill_id: String,
    }
    let _ = emit(
        &app,
        "pill:dismissed",
        Dismiss {
            pill_id: pill_id.clone(),
        },
    );
    #[derive(Serialize, Clone)]
    struct Unpinned {
        pill_id: String,
    }
    emit(&app, "pill:unpinned", Unpinned { pill_id }).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pinned_list(
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<Vec<PinnedPill>, String> {
    let db = {
        let proj = state.project.lock().await;
        proj.as_ref().ok_or("no project open")?.db.clone()
    };
    let g = db.lock().await;
    let mut stmt = g
        .conn()
        .prepare(
            "SELECT id, speaker_id, hue, message, COALESCE(rabbit_hole_path, ''), \
                    COALESCE(trigger_class, '') \
             FROM pinned_pill \
             ORDER BY pinned_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(PinnedPill {
                pill_id: r.get(0)?,
                speaker_id: r.get(1)?,
                hue_token: r.get(2)?,
                text: r.get(3)?,
                block_target_id: {
                    let s: String = r.get(4)?;
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                },
                trigger_id: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
