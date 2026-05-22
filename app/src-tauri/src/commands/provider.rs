use crate::events::{emit, ProviderStatusPayload};
use crate::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, State};
use water_core::llm::{
    AnthropicProvider, BouquetRequest, CannedProvider, GeminiProvider, KimiProvider,
    LlamaCppProvider, LlmProvider, LlmRouter, OllamaProvider, OpenAiProvider, OpenRouterProvider,
    ProviderId, Secrets,
};

#[tauri::command]
pub async fn provider_test(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: String,
    model: Option<String>,
) -> Result<Vec<String>, String> {
    let provider = build_provider(&provider_id)?;
    // Clone the Arc so we can both run the test and persist a router that
    // uses the SAME provider instance.
    let router = Arc::new(LlmRouter::new(vec![provider.clone()]));

    // Resolve the model to test against. The renderer passes the writer's
    // active selection (curated entry or custom string); empty/None falls
    // back to the adapter's hardcoded default so a writer who hasn't
    // touched the picker still hits a sensible default.
    let chosen_model = model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_model_for(&provider_id));

    // Subscribe to router status BEFORE the generate call so we don't miss
    // the transition. The forwarder task self-terminates when the channel
    // closes (router dropped) or on `Lagged`/`Closed` recv errors.
    let mut rx = router.subscribe_status();
    let app_clone = app.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(change) => {
                    let _ = emit(
                        &app_clone,
                        "provider:status",
                        ProviderStatusPayload {
                            provider_id: change.provider_id,
                            ok: change.ok,
                            error: change.error,
                        },
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let req = BouquetRequest {
        system: "You are testing the provider. Be reactive and concise.".into(),
        user: "Return three angles on the act of looking out of a window.".into(),
        n_variants: 3,
        previous_variants_first_words: vec![],
        model: chosen_model.clone(),
        temperature: 0.7,
        max_output_tokens: 200,
    };
    let (_id, variants) = router
        .generate_bouquet(&req)
        .await
        .map_err(|e| e.to_string())?;
    // Apply the chosen model as the default override on the new router so
    // subsequent single-shot calls (level-0 pills, rabbit fan, editor
    // polish — all of which use `generate_raw_with_default`) hit the same
    // model the writer just tested. Without this, the router would fall
    // back to each adapter's hardcoded default and the writer's selection
    // would silently lose effect after every Test click.
    router
        .set_default_model(&ProviderId::new(&provider_id), &chosen_model)
        .await;
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

/// Set the active model override for a provider on the live router.
/// Empty `model` clears the override (falls back to the provider's
/// hardcoded default). The renderer persists the choice in its own
/// localStorage and re-applies via this IPC on app boot, so the
/// override survives restarts without a server-side schema bump.
#[tauri::command]
pub async fn provider_set_model(
    state: State<'_, AppState>,
    provider_id: String,
    model: String,
) -> Result<(), String> {
    let router_arc = {
        let g = state.router.lock().await;
        g.clone()
    };
    let Some(router) = router_arc else {
        // No active router yet — nothing to override. The renderer
        // will call this again after `provider_test` lands an
        // active router.
        return Ok(());
    };
    router
        .set_default_model(&ProviderId::new(&provider_id), &model)
        .await;
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
        "kimi" => {
            let key = secrets.get("kimi").map_err(|e| e.to_string())?;
            Ok(Arc::new(KimiProvider::new(key)))
        }
        "openrouter" => {
            let key = secrets.get("openrouter").map_err(|e| e.to_string())?;
            Ok(Arc::new(OpenRouterProvider::new(key)))
        }
        "gemini" => {
            let key = secrets.get("gemini").map_err(|e| e.to_string())?;
            Ok(Arc::new(GeminiProvider::new(key)))
        }
        "ollama" => Ok(Arc::new(OllamaProvider::default_url())),
        "llamacpp" => Ok(Arc::new(LlamaCppProvider::new("http://127.0.0.1:8080"))),
        other => Err(format!("unknown provider: {other}")),
    }
}

fn default_model_for(provider_id: &str) -> String {
    match provider_id {
        // Claude Sonnet 4.6 — current production-tier model. Used both by
        // the provider Test button and as the default `BouquetRequest.model`
        // when the renderer doesn't override it. Cheap-enough for the test
        // bouquet (3 short variants); quality-good-enough for level-0 pill
        // text. Update when Anthropic publishes a newer Sonnet generation.
        "anthropic" => "claude-sonnet-4-6".into(),
        "openai" => "gpt-4o-mini".into(),
        // Kimi K2 (256k context) is the reason a writer would pick
        // Moonshot — enough headroom to embed whole drafts in a
        // single prompt rather than stitch excerpts. The provider
        // test button uses this same model so the writer's first
        // successful call exercises the long-context path.
        "kimi" => "kimi-k2-0905-preview".into(),
        // OpenRouter aggregates many providers; default to Kimi K2
        // because the long-context window is the main reason a
        // writer would route through OpenRouter. Writers can swap
        // via the Settings model picker.
        "openrouter" => "moonshotai/kimi-k2".into(),
        // Gemini 2.5 Flash — fast + cheap; the curated default a
        // tester is most likely to have access to. They can swap to
        // 2.5 Pro for higher-quality pills via the Model picker.
        "gemini" => "gemini-2.5-flash".into(),
        "ollama" => "qwen2.5:3b".into(),
        "llamacpp" => "default".into(),
        _ => "canned".into(),
    }
}
