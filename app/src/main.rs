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

use feature_registry::{FeatureId, FeatureMetadata, FeatureRegistry, PanelEvent, PanelLifecycle};
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent};

use serde::Serialize;

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
        ])
        .setup(|app| {
            // Register system-level global shortcut: Alt+Space toggles
            // the main window visibility.
            let handle = app.handle().clone();
            app.global_shortcut().on_shortcut(
                Shortcut::parse("Alt+Space").unwrap(),
                move |_app, _shortcut, event| {
                    if event == ShortcutEvent::Pressed {
                        if let Some(window) = handle.get_webview_window("main") {
                            let _ = if window.is_visible().unwrap_or(false) {
                                window.hide()
                            } else {
                                window.show()
                            };
                        }
                    }
                },
            )?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
