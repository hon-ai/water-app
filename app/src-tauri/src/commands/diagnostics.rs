use crate::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct DiagnosticsStatus {
    pub has_open_project: bool,
    pub project_root: Option<String>,
    pub router_primary_id: Option<String>,
    pub sidecar: Option<SidecarInfo>,
    pub provider_health: Vec<ProviderHealth>,
}

#[derive(Serialize)]
pub struct SidecarInfo {
    pub base_url: String,
    pub status: String,
    pub last_status_detail: Option<String>,
}

#[derive(Serialize)]
pub struct ProviderHealth {
    pub id: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn diagnostics_status(state: State<'_, AppState>) -> Result<DiagnosticsStatus, String> {
    // Read everything we need from the locks up front so we can drop them
    // before the (potentially slow) router.health() roundtrip.
    let (has, root, sidecar_info) = {
        let proj = state.project.lock().await;
        let has = proj.is_some();
        let root = proj.as_ref().map(|p| p.root.to_string_lossy().to_string());
        let sidecar_info = match proj.as_ref() {
            Some(p) => match (&p.sidecar, &p.supervisor) {
                (Some(sc), Some(sup)) => {
                    let evt = sup.current();
                    Some(SidecarInfo {
                        base_url: sc.base_url().to_string(),
                        status: match evt.status {
                            water_core::SidecarStatus::Loading => "loading".to_string(),
                            water_core::SidecarStatus::Ready => "ready".to_string(),
                            water_core::SidecarStatus::Error => "error".to_string(),
                        },
                        last_status_detail: evt.detail,
                    })
                }
                _ => None,
            },
            None => None,
        };
        (has, root, sidecar_info)
    };

    let router_arc = {
        let router = state.router.lock().await;
        router.clone()
    };

    let (router_primary_id, provider_health) = match router_arc {
        Some(r) => {
            let primary = r.primary_id().map(|id| id.as_str().to_string());
            let healths = r.health().await;
            let pheaths = healths
                .into_iter()
                .map(|(id, result)| ProviderHealth {
                    id: id.as_str().to_string(),
                    ok: result.is_ok(),
                    error: result.err(),
                })
                .collect();
            (primary, pheaths)
        }
        None => (None, Vec::new()),
    };

    Ok(DiagnosticsStatus {
        has_open_project: has,
        project_root: root,
        router_primary_id,
        sidecar: sidecar_info,
        provider_health,
    })
}
