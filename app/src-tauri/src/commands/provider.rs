use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use water_core::llm::{
    AnthropicProvider, BouquetRequest, CannedProvider, LlamaCppProvider, LlmProvider, LlmRouter,
    OllamaProvider, OpenAiProvider, Secrets,
};

#[tauri::command]
pub async fn provider_test(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<String>, String> {
    let provider = build_provider(&provider_id)?;
    // Clone the Arc so we can both run the test and persist a router that
    // uses the SAME provider instance.
    let router = Arc::new(LlmRouter::new(vec![provider.clone()]));
    let req = BouquetRequest {
        system: "You are testing the provider. Be reactive and concise.".into(),
        user: "Return three angles on the act of looking out of a window.".into(),
        n_variants: 3,
        previous_variants_first_words: vec![],
        model: default_model_for(&provider_id),
        temperature: 0.7,
        max_output_tokens: 200,
    };
    let (_id, variants) = router
        .generate_bouquet(&req)
        .await
        .map_err(|e| e.to_string())?;
    // Persist the router that actually contains the tested provider — not a canned stand-in.
    let mut g = state.router.lock().await;
    *g = Some(router);
    Ok(variants.into_iter().map(|v| v.text).collect())
}

#[tauri::command]
pub async fn provider_set_key(
    _state: State<'_, AppState>,
    provider_id: String,
    key: String,
) -> Result<(), String> {
    let s = Secrets::load();
    s.set(&provider_id, &key).map_err(|e| e.to_string())?;
    Ok(())
}

fn build_provider(provider_id: &str) -> Result<Arc<dyn LlmProvider>, String> {
    let secrets = Secrets::load();
    match provider_id {
        "canned" => Ok(Arc::new(CannedProvider::default())),
        "anthropic" => {
            let key = secrets.get("anthropic").map_err(|e| e.to_string())?;
            Ok(Arc::new(AnthropicProvider::new(key)))
        }
        "openai" => {
            let key = secrets.get("openai").map_err(|e| e.to_string())?;
            Ok(Arc::new(OpenAiProvider::new(key)))
        }
        "ollama" => Ok(Arc::new(OllamaProvider::default_url())),
        "llamacpp" => Ok(Arc::new(LlamaCppProvider::new("http://127.0.0.1:8080"))),
        other => Err(format!("unknown provider: {other}")),
    }
}

fn default_model_for(provider_id: &str) -> String {
    match provider_id {
        "anthropic" => "claude-3-5-sonnet-latest".into(),
        "openai" => "gpt-4o-mini".into(),
        "ollama" => "qwen2.5:3b".into(),
        "llamacpp" => "default".into(),
        _ => "canned".into(),
    }
}
