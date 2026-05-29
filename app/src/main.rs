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
    ProviderTestResult, UpdateProviderConfig,
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
        store
            .ai_provider(provider_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("provider '{provider_id}' not found"))?
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

#[tauri::command]
fn active_provider_get(state: tauri::State<'_, ProviderState>) -> Result<Option<ActiveProvider>, String> {
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
    store.set_active(provider_id, &model).map_err(|e| e.to_string())
}

#[tauri::command]
fn active_provider_clear(state: tauri::State<'_, ProviderState>) -> Result<bool, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.clear_active().map_err(|e| e.to_string())
}

// ── Entry point ───────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Mutex::new(AppState::new()))
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
            active_provider_get,
            active_provider_set,
            active_provider_clear,
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
}
