//! Tauri application binary — desktop shell entry point.
//!
//! Registers workspace panel lifecycle commands that call into the
//! `feature-registry` crate for feature metadata and panel state
//! tracking. Also registers a system-level global shortcut (Alt+Space)
//! to show/hide the app window via `tauri-plugin-global-shortcut`.

// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::sync::Mutex;

use ai_providers::{
    ActiveProvider, ChatMessage, ModelInfo, NewProviderConfig, ProviderConfig, ProviderStore,
    ProviderTestError, ProviderTestResult, TestConnectionErrorKind, UpdateProviderConfig,
};
use feature_registry::{FeatureId, FeatureMetadata, FeatureRegistry, PanelEvent, PanelLifecycle};
use stream_utils::Channel;
use tauri::{Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use serde::{Deserialize, Serialize};

// ── Panel info (returned to frontend) ─────────────────────────────

/// Information about an open panel, returned to the frontend.
#[derive(Debug, Clone, Serialize)]
struct PanelInfo {
    panel_id: String,
    feature_id: String,
    feature_name: String,
    feature_icon: String,
    state: String,
    size: Option<f64>,
}

// ── Shared app state ──────────────────────────────────────────────

struct AppState {
    registry: FeatureRegistry,
    panels: HashMap<String, PanelLifecycle>,
    panel_meta: HashMap<String, (FeatureId, f64)>, // panel_id → (feature_id, size)
    next_id: u64,
}

impl AppState {
    fn new() -> Self {
        let mut registry = FeatureRegistry::new();

        // Register built-in features.
        for (id, name, icon) in [
            ("editor", "Editor", "📝"),
            ("terminal", "Terminal", "💻"),
            ("chat", "Chat", "💬"),
            ("git", "Git", "🔀"),
            ("settings", "Settings", "⚙️"),
        ] {
            let _ = registry.register(FeatureMetadata {
                id: FeatureId::new(id),
                name: name.into(),
                icon: icon.into(),
                capability_file: format!("capabilities/{id}.json").into(),
            });
        }

        Self {
            registry,
            panels: HashMap::new(),
            panel_meta: HashMap::new(),
            next_id: 1,
        }
    }

    fn next_panel_id(&mut self) -> String {
        let id = format!("panel-{}", self.next_id);
        self.next_id += 1;
        id
    }
}

struct ProviderState {
    store: Mutex<ProviderStore>,
}

/// Cancellation state for the active chat stream.
///
/// When `chat_send_stream` starts, it stores a `oneshot::Sender` here
/// along with a monotonically increasing generation id. The
/// `chat_cancel` command fires that sender, causing `tokio::select!`
/// to drop the stream future (which closes the HTTP connection).
///
/// The generation id guards against a stale clear: when a stream
/// finishes naturally it only clears the slot if the slot still holds
/// *its own* generation. A newer stream that registered in the meantime
/// is left untouched, so `chat_cancel` keeps working on the latest one.
struct ChatStreamState {
    slot: Mutex<ChatStreamSlot>,
}

struct ChatStreamSlot {
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    generation: u64,
}

impl ChatStreamState {
    fn new() -> Self {
        Self {
            slot: Mutex::new(ChatStreamSlot {
                cancel: None,
                generation: 0,
            }),
        }
    }

    fn register_cancel(&self, tx: tokio::sync::oneshot::Sender<()>) -> Result<u64, String> {
        let mut slot = self.slot.lock().map_err(|e| e.to_string())?;
        slot.generation = slot.generation.saturating_add(1);
        slot.cancel = Some(tx);
        Ok(slot.generation)
    }

    fn clear_cancel_if_generation(&self, generation: u64) -> Result<(), String> {
        let mut slot = self.slot.lock().map_err(|e| e.to_string())?;
        if slot.generation == generation {
            slot.cancel = None;
        }
        Ok(())
    }

    fn cancel_active(&self) -> Result<bool, String> {
        let mut slot = self.slot.lock().map_err(|e| e.to_string())?;
        if let Some(tx) = slot.cancel.take() {
            let _ = tx.send(());
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl ProviderState {
    fn new<R: Runtime>(app: &tauri::App<R>) -> Result<Self, String> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("failed to resolve app data dir: {e}"))?;
        let database_path = data_dir.join("openspace.sqlite3");

        let store = ProviderStore::open(database_path, data_dir)
            .map_err(|e| format!("failed to open provider store: {e}"))?;

        // Seed default (key-less) provider profiles on first run.
        store
            .seed_default_profiles()
            .map_err(|e| format!("failed to seed default provider profiles: {e}"))?;

        Ok(Self {
            store: Mutex::new(store),
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderWritePayload {
    name: String,
    base_url: String,
    api_key: Option<String>,
    auth_header_name: Option<String>,
    auth_header_value_prefix: Option<String>,
    models: Vec<ModelInfo>,
    request_body_template: serde_json::Value,
    response_path: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderChatPayload {
    provider_id: i64,
    model: String,
    messages: Vec<ChatMessage>,
    temperature: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
struct ProviderResponse {
    id: i64,
    name: String,
    base_url: String,
    api_key_redacted: String,
    has_api_key: bool,
    auth_header_name: String,
    auth_header_value_prefix: String,
    models: Vec<ModelInfo>,
    request_body_template: serde_json::Value,
    response_path: String,
}

impl From<ProviderConfig> for ProviderResponse {
    fn from(provider: ProviderConfig) -> Self {
        let has_api_key = provider.has_api_key();

        Self {
            id: provider.id,
            name: provider.name,
            base_url: provider.base_url,
            api_key_redacted: "[REDACTED]".to_string(),
            has_api_key,
            auth_header_name: provider.auth_header_name,
            auth_header_value_prefix: provider.auth_header_value_prefix,
            models: provider.models,
            request_body_template: provider.request_body_template,
            response_path: provider.response_path,
        }
    }
}

// ── Tauri commands ────────────────────────────────────────────────

#[tauri::command]
fn open_panel(
    feature_id: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<PanelInfo, String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    let fid = FeatureId::new(&feature_id);

    let meta = app
        .registry
        .get(&fid)
        .ok_or_else(|| format!("feature '{feature_id}' not registered"))?;

    // Clone metadata fields before mutable borrows below.
    let feature_name = meta.name.clone();
    let feature_icon = meta.icon.clone();
    let fid_clone = fid.clone();

    let panel_id = app.next_panel_id();
    let mut lifecycle = PanelLifecycle::new();

    lifecycle
        .transition(PanelEvent::Open)
        .map_err(|e| e.to_string())?;

    app.panels.insert(panel_id.clone(), lifecycle);
    app.panel_meta.insert(panel_id.clone(), (fid_clone, 400.0));

    Ok(PanelInfo {
        panel_id,
        feature_id: fid.to_string(),
        feature_name,
        feature_icon,
        state: "Opened".into(),
        size: Some(400.0),
    })
}

#[tauri::command]
fn close_panel(panel_id: String, state: tauri::State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;

    let lifecycle = app
        .panels
        .get_mut(&panel_id)
        .ok_or_else(|| format!("panel '{panel_id}' not found"))?;

    lifecycle
        .transition(PanelEvent::Close)
        .map_err(|e| e.to_string())?;

    app.panels.remove(&panel_id);
    app.panel_meta.remove(&panel_id);

    Ok(())
}

#[tauri::command]
fn focus_panel(panel_id: String, state: tauri::State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;

    let lifecycle = app
        .panels
        .get_mut(&panel_id)
        .ok_or_else(|| format!("panel '{panel_id}' not found"))?;

    lifecycle
        .transition(PanelEvent::Focus)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn resize_panel(
    panel_id: String,
    size: f64,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let app = state.lock().map_err(|e| e.to_string())?;

    // Verify the panel exists.
    let _ = app
        .panel_meta
        .get(&panel_id)
        .ok_or_else(|| format!("panel '{panel_id}' not found"))?;

    // Drop the read lock before acquiring write lock.
    drop(app);
    let mut app = state.lock().map_err(|e| e.to_string())?;

    if let Some((_, s)) = app.panel_meta.get_mut(&panel_id) {
        *s = size.max(100.0);
    }

    Ok(())
}

#[tauri::command]
fn list_features(state: tauri::State<'_, Mutex<AppState>>) -> Result<Vec<FeatureMetadata>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    Ok(app.registry.list().into_iter().cloned().collect())
}

#[tauri::command]
fn provider_create(
    payload: ProviderWritePayload,
    state: tauri::State<'_, ProviderState>,
) -> Result<i64, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;

    store
        .create(NewProviderConfig {
            name: payload.name,
            base_url: payload.base_url,
            api_key: payload.api_key,
            auth_header_name: payload.auth_header_name,
            auth_header_value_prefix: payload.auth_header_value_prefix,
            models: payload.models,
            request_body_template: payload.request_body_template,
            response_path: payload.response_path,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn provider_get(
    id: i64,
    state: tauri::State<'_, ProviderState>,
) -> Result<Option<ProviderResponse>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .get(id)
        .map(|provider| provider.map(Into::into))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn provider_list(state: tauri::State<'_, ProviderState>) -> Result<Vec<ProviderResponse>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .list()
        .map(|providers| providers.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn provider_update(
    id: i64,
    payload: ProviderWritePayload,
    state: tauri::State<'_, ProviderState>,
) -> Result<bool, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .update(
            id,
            UpdateProviderConfig {
                name: payload.name,
                base_url: payload.base_url,
                api_key: payload.api_key,
                auth_header_name: payload.auth_header_name,
                auth_header_value_prefix: payload.auth_header_value_prefix,
                models: payload.models,
                request_body_template: payload.request_body_template,
                response_path: payload.response_path,
            },
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn provider_delete(id: i64, state: tauri::State<'_, ProviderState>) -> Result<bool, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.delete(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn provider_test_connection(
    provider_id: i64,
    state: tauri::State<'_, ProviderState>,
) -> Result<ProviderTestResult, String> {
    let provider = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        match store.ai_provider(provider_id) {
            Ok(Some(provider)) => provider,
            Ok(None) => {
                return Ok(ProviderTestResult {
                    success: false,
                    error: Some(ProviderTestError {
                        kind: TestConnectionErrorKind::InvalidConfig,
                        message: format!("provider '{provider_id}' not found"),
                    }),
                });
            }
            Err(err) => {
                return Ok(ProviderTestResult {
                    success: false,
                    error: Some(ProviderTestError {
                        kind: TestConnectionErrorKind::InvalidConfig,
                        message: err.to_string(),
                    }),
                });
            }
        }
    };

    Ok(provider.test_connection().await)
}

#[tauri::command]
async fn provider_chat_stream(
    payload: ProviderChatPayload,
    on_token: tauri::ipc::Channel<String>,
    state: tauri::State<'_, ProviderState>,
) -> Result<(), String> {
    let provider = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store
            .ai_provider(payload.provider_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("provider '{}' not found", payload.provider_id))?
    };
    let channel = Channel::from_tauri(on_token);

    provider
        .chat_stream(
            &payload.model,
            &payload.messages,
            payload.temperature.unwrap_or(0.7),
            &channel,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Stream a chat completion using the **active** provider and model.
///
/// The chat feature calls this command so it never needs to know which
/// provider is selected — it simply passes user messages and receives
/// tokens through the channel.
///
/// The stream can be cancelled via [`chat_cancel`], which fires a
/// `oneshot` token. `tokio::select!` races the stream against the
/// cancel signal; when cancelled, the stream future is dropped, which
/// drops the `reqwest::Response` and closes the HTTP connection.
#[tauri::command]
async fn chat_send_stream(
    messages: Vec<ChatMessage>,
    temperature: Option<f64>,
    on_token: tauri::ipc::Channel<String>,
    provider_state: tauri::State<'_, ProviderState>,
    stream_state: tauri::State<'_, ChatStreamState>,
) -> Result<(), String> {
    let (provider, model) = {
        let store = provider_state.store.lock().map_err(|e| e.to_string())?;

        let active = store.get_active().map_err(|e| e.to_string())?;
        let active = active.ok_or(
            "No active provider configured. Open Settings → Providers to choose a provider and model.",
        )?;

        let provider = store
            .ai_provider(active.provider_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!(
                    "Active provider '{}' not found. It may have been deleted. \
                     Open Settings → Providers to select a new one.",
                    active.provider_id
                )
            })?;

        (provider, active.model)
    };

    let channel = Channel::from_tauri(on_token);
    let temp = temperature.unwrap_or(0.7);

    // Register a cancellation token so `chat_cancel` can stop the stream.
    // Each registration bumps the generation under the same mutex as the
    // sender write, so the generation always identifies the sender currently
    // in the slot.
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    let my_generation = stream_state.register_cancel(cancel_tx)?;

    let result = tokio::select! {
        result = provider.chat_stream(&model, &messages, temp, &channel) => {
            result.map_err(|e| e.to_string())
        }
        _ = cancel_rx => {
            // Graceful cancellation — the stream future is dropped here,
            // which drops the reqwest::Response and closes the HTTP connection.
            Ok(())
        }
    };

    // Clear the cancel token — but only if we still own the slot. A newer
    // stream may have registered (and bumped the generation) while this one
    // was finishing; in that case we must not null out its token.
    stream_state.clear_cancel_if_generation(my_generation)?;

    result
}

/// Cancel the currently active chat stream, if any.
///
/// Returns `true` if a stream was cancelled, `false` if no stream was active.
#[tauri::command]
fn chat_cancel(stream_state: tauri::State<'_, ChatStreamState>) -> Result<bool, String> {
    stream_state.cancel_active()
}

#[tauri::command]
fn active_provider_get(
    state: tauri::State<'_, ProviderState>,
) -> Result<Option<ActiveProvider>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.get_active().map_err(|e| e.to_string())
}

#[tauri::command]
fn active_provider_set(
    provider_id: i64,
    model: String,
    state: tauri::State<'_, ProviderState>,
) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .set_active(provider_id, &model)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn active_provider_clear(state: tauri::State<'_, ProviderState>) -> Result<bool, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.clear_active().map_err(|e| e.to_string())
}

// ── Inline completion ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct InlineCompletionRequest {
    /// The full document text surrounding the cursor.
    document: String,
    /// Zero-based cursor offset in the document.
    cursor_position: u32,
}

#[derive(Debug, Clone, Serialize)]
struct InlineCompletionResponse {
    /// The completion text suggested by the provider.
    completion: String,
    /// The provider name that produced this completion.
    provider_name: String,
    /// The model that produced this completion.
    model: String,
}

/// Request inline code completion using the **active** provider and model.
///
/// Sends the document context through the same generic provider adapter
/// used by chat. Inline completion does not need to know which provider
/// is behind the adapter — it simply passes document context and receives
/// a completion string.
#[tauri::command]
async fn inline_completion(
    request: InlineCompletionRequest,
    state: tauri::State<'_, ProviderState>,
) -> Result<InlineCompletionResponse, String> {
    let (provider, model, provider_name) = {
        let store = state.store.lock().map_err(|e| e.to_string())?;

        let active = store.get_active().map_err(|e| e.to_string())?;
        let active = active.ok_or(
            "No active provider configured. Open Settings → Providers to choose a provider and model.",
        )?;

        let provider = store
            .ai_provider(active.provider_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!(
                    "Active provider '{}' not found. It may have been deleted. \
                     Open Settings → Providers to select a new one.",
                    active.provider_id
                )
            })?;

        let provider_name = provider.config().name.clone();
        (provider, active.model, provider_name)
    };

    let document = request.document;
    let cursor = request.cursor_position as usize;
    let before_cursor = &document[..cursor.min(document.len())];
    let after_cursor = &document[cursor.min(document.len())..];

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You are a code completion assistant. Given the code before and after the cursor, provide the most likely completion text. Return ONLY the completion text, nothing else. Do not repeat code that is already present before the cursor.".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "Code before cursor:\n```\n{before}\n```\n\nCode after cursor:\n```\n{after}\n```\n\nComplete the code at the cursor position.",
                before = before_cursor,
                after = after_cursor,
            ),
        },
    ];

    let completion = provider
        .chat_once(&model, &messages, 0.3)
        .await
        .map_err(|e| e.to_string())?;

    Ok(InlineCompletionResponse {
        completion,
        provider_name,
        model,
    })
}

// ── Entry point ───────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Mutex::new(AppState::new()))
        .manage(ChatStreamState::new())
        .invoke_handler(tauri::generate_handler![
            open_panel,
            close_panel,
            focus_panel,
            resize_panel,
            list_features,
            provider_create,
            provider_get,
            provider_list,
            provider_update,
            provider_delete,
            provider_test_connection,
            provider_chat_stream,
            chat_send_stream,
            chat_cancel,
            active_provider_get,
            active_provider_set,
            active_provider_clear,
            inline_completion,
        ])
        .setup(|app| {
            app.manage(ProviderState::new(app)?);
            // Register system-level global shortcut: Alt+Space toggles
            // the main window visibility.
            let handle = app.handle().clone();
            app.global_shortcut()
                .on_shortcut("Alt+Space", move |_app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Some(window) = handle.get_webview_window("main") {
                            let _ = if window.is_visible().unwrap_or(false) {
                                window.hide()
                            } else {
                                window.show()
                            };
                        }
                    }
                })?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_with_key(api_key_encrypted: Option<Vec<u8>>) -> ProviderConfig {
        ProviderConfig {
            id: 1,
            name: "Test".to_string(),
            base_url: "https://example.com".to_string(),
            api_key_encrypted,
            auth_header_name: "Authorization".to_string(),
            auth_header_value_prefix: "Bearer ".to_string(),
            models: vec![],
            request_body_template: serde_json::json!({}),
            response_path: "choices.0.message.content".to_string(),
        }
    }

    #[test]
    fn provider_response_exposes_has_api_key_true_when_key_present() {
        let response = ProviderResponse::from(provider_with_key(Some(vec![1, 2, 3])));
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["has_api_key"], serde_json::json!(true));
    }

    #[test]
    fn provider_response_marks_keyless_provider_unusable() {
        let none = ProviderResponse::from(provider_with_key(None));
        let empty = ProviderResponse::from(provider_with_key(Some(vec![])));
        assert!(!none.has_api_key);
        assert!(!empty.has_api_key);
    }

    #[test]
    fn provider_response_never_serializes_real_api_key() {
        let response = ProviderResponse::from(provider_with_key(Some(vec![1, 2, 3])));
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["api_key_redacted"], serde_json::json!("[REDACTED]"));
        assert!(json.get("api_key_encrypted").is_none());
    }

    // ── chat_send_stream integration tests ──────────────────────

    /// Core chat logic extracted for direct testing without Tauri command
    /// wrapper overhead. This mirrors what `chat_send_stream` does:
    /// read active → resolve provider → stream through adapter.
    async fn execute_chat(
        store: &ProviderStore,
        messages: Vec<ChatMessage>,
        temperature: f64,
        channel: &Channel<String>,
    ) -> Result<(), String> {
        let active = store
            .get_active()
            .map_err(|e| e.to_string())?
            .ok_or("No active provider configured")?;

        let provider = store
            .ai_provider(active.provider_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("provider '{}' not found", active.provider_id))?;

        provider
            .chat_stream(&active.model, &messages, temperature, channel)
            .await
            .map_err(|e| e.to_string())
    }

    #[tokio::test]
    async fn chat_streams_tokens_through_active_provider() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(concat!(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
                        "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
                        "data: [DONE]\n\n"
                    )),
            )
            .mount(&server)
            .await;

        let test_dir = tempfile::tempdir().unwrap();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let provider_id = store
            .create(ai_providers::NewProviderConfig {
                name: "Test".to_string(),
                base_url: format!("{}/v1/chat/completions", server.uri()),
                api_key: Some("sk-test".to_string()),
                auth_header_name: None,
                auth_header_value_prefix: None,
                models: vec![ModelInfo {
                    id: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    context_window: 128000,
                }],
                request_body_template: serde_json::json!({
                    "model": "{model}",
                    "messages": "{messages}",
                    "stream": "{stream}",
                    "temperature": "{temperature}"
                }),
                response_path: "choices[0].delta.content".to_string(),
            })
            .unwrap();

        store.set_active(provider_id, "gpt-4o").unwrap();

        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let tauri_channel = tauri::ipc::Channel::<String>::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let item: String = serde_json::from_str(&s).unwrap();
                received_clone.lock().unwrap().push(item);
            }
            Ok(())
        });
        let channel = Channel::from_tauri(tauri_channel);

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Say hello".to_string(),
        }];

        execute_chat(&store, messages, 0.7, &channel).await.unwrap();

        assert_eq!(
            *received.lock().unwrap(),
            vec!["Hello".to_string(), " world".to_string()]
        );
    }

    #[tokio::test]
    async fn chat_returns_clear_error_when_no_active_provider() {
        let test_dir = tempfile::tempdir().unwrap();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();
        store.seed_default_profiles().unwrap();

        let tauri_channel = tauri::ipc::Channel::<String>::new(|_| Ok(()));
        let channel = Channel::from_tauri(tauri_channel);

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];

        let error = execute_chat(&store, messages, 0.7, &channel)
            .await
            .unwrap_err();

        assert!(
            error.contains("No active provider"),
            "Expected clear no-provider message, got: {error}"
        );
    }

    #[tokio::test]
    async fn chat_produces_no_tokens_when_response_path_does_not_match() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        // Return a streaming response whose structure doesn't match the
        // configured response_path. The adapter silently drops each token
        // that fails extraction (normal SSE behaviour), so the stream
        // completes with zero tokens rather than surfacing a hard error.
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(concat!(
                        "data: {\"unexpected_structure\": true}\n\n",
                        "data: [DONE]\n\n"
                    )),
            )
            .mount(&server)
            .await;

        let test_dir = tempfile::tempdir().unwrap();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let provider_id = store
            .create(ai_providers::NewProviderConfig {
                name: "Broken".to_string(),
                base_url: format!("{}/v1/chat/completions", server.uri()),
                api_key: Some("sk-test".to_string()),
                auth_header_name: None,
                auth_header_value_prefix: None,
                models: vec![ModelInfo {
                    id: "model-x".to_string(),
                    name: "Model X".to_string(),
                    context_window: 4096,
                }],
                request_body_template: serde_json::json!({
                    "model": "{model}",
                    "messages": "{messages}",
                    "stream": "{stream}"
                }),
                response_path: "choices[0].message.content".to_string(),
            })
            .unwrap();

        store.set_active(provider_id, "model-x").unwrap();

        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let tauri_channel = tauri::ipc::Channel::<String>::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let item: String = serde_json::from_str(&s).unwrap();
                received_clone.lock().unwrap().push(item);
            }
            Ok(())
        });
        let channel = Channel::from_tauri(tauri_channel);

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];

        // Stream completes successfully but no tokens are extracted.
        execute_chat(&store, messages, 0.7, &channel).await.unwrap();

        assert!(
            received.lock().unwrap().is_empty(),
            "Expected zero tokens when response path does not match"
        );
    }

    // ── Cancellation tests ────────────────────────────────────────

    /// Resolve the active provider from the store, returning the owned
    /// `AiProvider` and model string. Panics if no active provider is set.
    fn resolve_active(store: &ProviderStore) -> (ai_providers::AiProvider, String) {
        let active = store
            .get_active()
            .expect("get_active failed")
            .expect("no active provider");
        let provider = store
            .ai_provider(active.provider_id)
            .expect("ai_provider failed")
            .expect("provider not found");
        (provider, active.model)
    }

    #[tokio::test]
    async fn chat_cancel_returns_false_when_no_stream_active() {
        let state = ChatStreamState::new();
        assert!(
            !state.cancel_active().unwrap(),
            "expected false when no stream is active"
        );
    }

    #[tokio::test]
    async fn chat_cancel_returns_true_when_stream_is_active() {
        let state = ChatStreamState::new();
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        state.register_cancel(cancel_tx).unwrap();

        assert!(
            state.cancel_active().unwrap(),
            "expected true when a stream is active"
        );
        assert!(
            cancel_rx.await.is_ok(),
            "expected registered cancel receiver to be fired"
        );
    }

    #[tokio::test]
    async fn stale_stream_cleanup_does_not_clear_newer_cancel_token() {
        let state = ChatStreamState::new();
        let (first_tx, _first_rx) = tokio::sync::oneshot::channel::<()>();
        let first_generation = state.register_cancel(first_tx).unwrap();

        let (second_tx, second_rx) = tokio::sync::oneshot::channel::<()>();
        state.register_cancel(second_tx).unwrap();

        // The first stream finishes after a newer stream registered. Its
        // cleanup must not clear the newer stream's token.
        state.clear_cancel_if_generation(first_generation).unwrap();

        assert!(
            state.cancel_active().unwrap(),
            "newer cancel token should remain active after stale cleanup"
        );
        assert!(
            second_rx.await.is_ok(),
            "expected newer cancel receiver to be fired"
        );
    }

    #[tokio::test]
    async fn chat_stream_cancellation_stops_reading() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // A mock server that responds with a large SSE stream. The cancel
        // signal fires before the adapter can process all events, so we
        // expect fewer tokens than the server sent.
        let server = MockServer::start().await;
        let sse_body: String = (0..100)
            .map(|i| {
                format!(
                    "data: {{\"choices\":[{{\"delta\":{{\"content\":\"t{i}\"}}}}]}}\n\n",
                    i = i
                )
            })
            .chain(std::iter::once("data: [DONE]\n\n".to_string()))
            .collect();

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse_body),
            )
            .mount(&server)
            .await;

        let test_dir = tempfile::tempdir().unwrap();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        store
            .create(ai_providers::NewProviderConfig {
                name: "Test".to_string(),
                base_url: format!("{}/v1/chat/completions", server.uri()),
                api_key: Some("sk-test".to_string()),
                auth_header_name: None,
                auth_header_value_prefix: None,
                models: vec![ModelInfo {
                    id: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    context_window: 128000,
                }],
                request_body_template: serde_json::json!({
                    "model": "{model}",
                    "messages": "{messages}",
                    "stream": "{stream}",
                    "temperature": "{temperature}"
                }),
                response_path: "choices[0].delta.content".to_string(),
            })
            .unwrap();

        store.set_active(1, "gpt-4o").unwrap();

        // Resolve the provider before spawning so we don't hold &ProviderStore
        // across a Send boundary (ProviderStore contains RefCell).
        let (provider, model) = resolve_active(&store);

        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let tauri_channel = tauri::ipc::Channel::<String>::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let item: String = serde_json::from_str(&s).unwrap();
                received_clone.lock().unwrap().push(item);
            }
            Ok(())
        });
        let channel = Channel::from_tauri(tauri_channel);

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];

        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();

        // Fire the cancel signal *before* spawning the stream. The first time
        // the spawned task is polled, `cancel_rx` is already ready, so
        // `tokio::select!` takes the cancel branch and drops the stream
        // future. This makes the truncation deterministic: the provider
        // stream is dropped before it can drain all 100 events.
        cancel_tx.send(()).unwrap();

        let handle = tokio::spawn(async move {
            tokio::select! {
                biased;
                _ = cancel_rx => {
                    Ok(()) // Graceful cancellation
                }
                result = provider.chat_stream(&model, &messages, 0.7, &channel) => {
                    result.map_err(|e| e.to_string())
                }
            }
        });

        let result = handle.await.unwrap();
        // Cancellation is graceful — no error.
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);

        // The stream was cancelled before completion, so strictly fewer than
        // all 100 tokens were delivered. (`biased` + pre-fired cancel ensures
        // the cancel branch wins the first poll.)
        let count = received.lock().unwrap().len();
        assert!(
            count < 100,
            "stream should have been cancelled before completion, got {count} tokens"
        );
    }

    #[tokio::test]
    async fn chat_stream_receiver_disconnect_stops_reading() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(concat!(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"A\"}}]}\n\n",
                        "data: {\"choices\":[{\"delta\":{\"content\":\"B\"}}]}\n\n",
                        "data: {\"choices\":[{\"delta\":{\"content\":\"C\"}}]}\n\n",
                        "data: [DONE]\n\n"
                    )),
            )
            .mount(&server)
            .await;

        let test_dir = tempfile::tempdir().unwrap();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        store
            .create(ai_providers::NewProviderConfig {
                name: "Test".to_string(),
                base_url: format!("{}/v1/chat/completions", server.uri()),
                api_key: Some("sk-test".to_string()),
                auth_header_name: None,
                auth_header_value_prefix: None,
                models: vec![ModelInfo {
                    id: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    context_window: 128000,
                }],
                request_body_template: serde_json::json!({
                    "model": "{model}",
                    "messages": "{messages}",
                    "stream": "{stream}",
                    "temperature": "{temperature}"
                }),
                response_path: "choices[0].delta.content".to_string(),
            })
            .unwrap();

        store.set_active(1, "gpt-4o").unwrap();

        // Channel that fails on the second send (simulating receiver disconnect).
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let send_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let send_count_clone = send_count.clone();

        let tauri_channel = tauri::ipc::Channel::<String>::new(move |body| {
            let count = send_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let item: String = serde_json::from_str(&s).unwrap();
                received_clone.lock().unwrap().push(item);
            }
            if count >= 2 {
                return Err(tauri::Error::WebviewNotFound);
            }
            Ok(())
        });
        let channel = Channel::from_tauri(tauri_channel);

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];

        let result = execute_chat(&store, messages, 0.7, &channel).await;

        // Stream should have returned a channel error.
        assert!(result.is_err(), "expected channel error from disconnect");

        // "A" was delivered, "B" collected before the callback returned Err.
        let received = received.lock().unwrap();
        assert_eq!(*received, vec!["A".to_string(), "B".to_string()]);
    }

    // ── Inline completion tests ──────────────────────────────────

    /// Core inline completion logic extracted for direct testing without Tauri
    /// command wrapper overhead. This mirrors what `inline_completion` does:
    /// read active → resolve provider → send a one-shot request through the
    /// generic adapter and return the completion text.
    async fn execute_inline_completion(
        store: &ProviderStore,
        document: &str,
        cursor_position: u32,
    ) -> Result<InlineCompletionResponse, String> {
        let (provider, model, provider_name) = {
            let active = store
                .get_active()
                .map_err(|e| e.to_string())?
                .ok_or("No active provider configured")?;

            let provider = store
                .ai_provider(active.provider_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("provider '{}' not found", active.provider_id))?;

            let provider_name = provider.config().name.clone();
            (provider, active.model, provider_name)
        };

        let cursor = cursor_position as usize;
        let before_cursor = &document[..cursor.min(document.len())];
        let after_cursor = &document[cursor.min(document.len())..];

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a code completion assistant. Given the code before and after the cursor, provide the most likely completion text. Return ONLY the completion text, nothing else. Do not repeat code that is already present before the cursor.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Code before cursor:\n```\n{before}\n```\n\nCode after cursor:\n```\n{after}\n```\n\nComplete the code at the cursor position.",
                    before = before_cursor,
                    after = after_cursor,
                ),
            },
        ];

        let completion = provider
            .chat_once(&model, &messages, 0.3)
            .await
            .map_err(|e| e.to_string())?;

        Ok(InlineCompletionResponse {
            completion,
            provider_name,
            model,
        })
    }

    #[tokio::test]
    async fn inline_completion_returns_completion_from_active_provider() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "let x = 42;"}}]
            })))
            .mount(&server)
            .await;

        let test_dir = tempfile::tempdir().unwrap();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let provider_id = store
            .create(ai_providers::NewProviderConfig {
                name: "Test".to_string(),
                base_url: format!("{}/v1/chat/completions", server.uri()),
                api_key: Some("sk-test".to_string()),
                auth_header_name: None,
                auth_header_value_prefix: None,
                models: vec![ModelInfo {
                    id: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    context_window: 128000,
                }],
                request_body_template: serde_json::json!({
                    "model": "{model}",
                    "messages": "{messages}",
                    "stream": "{stream}",
                    "temperature": "{temperature}"
                }),
                response_path: "choices[0].message.content".to_string(),
            })
            .unwrap();

        store.set_active(provider_id, "gpt-4o").unwrap();

        let response = execute_inline_completion(
            &store,
            "fn main() {\n    let x = ",
            23,
        )
        .await
        .unwrap();

        assert_eq!(response.completion, "let x = 42;");
        assert_eq!(response.provider_name, "Test");
        assert_eq!(response.model, "gpt-4o");
    }

    #[tokio::test]
    async fn inline_completion_uses_active_provider_not_hardcoded() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"text": "println!(\"hello\");"}]
            })))
            .mount(&server)
            .await;

        let test_dir = tempfile::tempdir().unwrap();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        // Create an Anthropic-like provider and make it active.
        let provider_id = store
            .create(ai_providers::NewProviderConfig {
                name: "Anthropic".to_string(),
                base_url: format!("{}/v1/messages", server.uri()),
                api_key: Some("sk-anthropic".to_string()),
                auth_header_name: Some("x-api-key".to_string()),
                auth_header_value_prefix: Some(String::new()),
                models: vec![ModelInfo {
                    id: "claude-sonnet".to_string(),
                    name: "Claude Sonnet".to_string(),
                    context_window: 200000,
                }],
                request_body_template: serde_json::json!({
                    "model": "{model}",
                    "messages": "{messages}",
                    "max_tokens": 1024,
                }),
                response_path: "content[0].text".to_string(),
            })
            .unwrap();

        store.set_active(provider_id, "claude-sonnet").unwrap();

        let response = execute_inline_completion(
            &store,
            "fn greet() {\n    ",
            17,
        )
        .await
        .unwrap();

        assert_eq!(response.completion, "println!(\"hello\");");
        assert_eq!(response.provider_name, "Anthropic");
        assert_eq!(response.model, "claude-sonnet");

        // Verify the request was sent with Anthropic's auth header.
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]
                .headers
                .get("x-api-key")
                .unwrap()
                .to_str()
                .unwrap(),
            "sk-anthropic"
        );
    }

    #[tokio::test]
    async fn inline_completion_returns_clear_error_when_no_active_provider() {
        let test_dir = tempfile::tempdir().unwrap();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();
        store.seed_default_profiles().unwrap();

        let error = execute_inline_completion(&store, "fn main() {}", 0)
            .await
            .unwrap_err();

        assert!(
            error.contains("No active provider"),
            "Expected clear no-provider message, got: {error}"
        );
    }

    #[tokio::test]
    async fn inline_completion_returns_error_when_response_path_does_not_match() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        // Return a response whose structure doesn't match the configured
        // response_path.
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "unexpected": "structure"
            })))
            .mount(&server)
            .await;

        let test_dir = tempfile::tempdir().unwrap();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        store
            .create(ai_providers::NewProviderConfig {
                name: "Test".to_string(),
                base_url: format!("{}/v1/chat/completions", server.uri()),
                api_key: Some("sk-test".to_string()),
                auth_header_name: None,
                auth_header_value_prefix: None,
                models: vec![ModelInfo {
                    id: "model-x".to_string(),
                    name: "Model X".to_string(),
                    context_window: 4096,
                }],
                request_body_template: serde_json::json!({
                    "model": "{model}",
                    "messages": "{messages}",
                    "stream": "{stream}",
                    "temperature": "{temperature}"
                }),
                response_path: "choices[0].message.content".to_string(),
            })
            .unwrap();

        store.set_active(1, "model-x").unwrap();

        let error = execute_inline_completion(&store, "code", 0)
            .await
            .unwrap_err();

        assert!(
            error.contains("response path"),
            "Expected response path error, got: {error}"
        );
    }

    #[tokio::test]
    async fn inline_completion_sends_cursor_context_to_provider() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "completed"}}]
            })))
            .mount(&server)
            .await;

        let test_dir = tempfile::tempdir().unwrap();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        store
            .create(ai_providers::NewProviderConfig {
                name: "Test".to_string(),
                base_url: format!("{}/v1/chat/completions", server.uri()),
                api_key: Some("sk-test".to_string()),
                auth_header_name: None,
                auth_header_value_prefix: None,
                models: vec![ModelInfo {
                    id: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    context_window: 128000,
                }],
                request_body_template: serde_json::json!({
                    "model": "{model}",
                    "messages": "{messages}",
                    "stream": "{stream}",
                    "temperature": "{temperature}"
                }),
                response_path: "choices[0].message.content".to_string(),
            })
            .unwrap();

        store.set_active(1, "gpt-4o").unwrap();

        let document = "fn main() {\n    let x = \n}";
        let cursor = 23; // after "let x = "
        execute_inline_completion(&store, document, cursor)
            .await
            .unwrap();

        // Verify the request body contains the cursor context.
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);

        let user_content = messages[1]["content"].as_str().unwrap();
        assert!(user_content.contains("Code before cursor:"), "Expected cursor context in user message, got: {user_content}");
        assert!(user_content.contains("let x ="), "Expected before-cursor code in prompt, got: {user_content}");
        assert!(user_content.contains("}"), "Expected after-cursor code in prompt, got: {user_content}");

        // Verify temperature is 0.3 (lower for completions).
        assert_eq!(body["temperature"], 0.3);
    }
}
