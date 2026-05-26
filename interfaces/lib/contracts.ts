/**
 * Contract types — TypeScript ↔ Rust alignment.
 *
 * These types mirror the Rust structs in `app/src/main.rs` and
 * `cores/feature-registry/src/lib.rs`. Keep them in sync when
 * either side changes.
 *
 * Rust sources:
 *   - PanelInfo:  app/src/main.rs
 *   - FeatureMetadata: cores/feature-registry/src/lib.rs
 *   - Tauri commands: app/src/main.rs (open_panel, close_panel, …)
 */

/** Matches Rust `PanelInfo` in `app/src/main.rs`. */
export interface PanelInfo {
  panel_id: string;
  feature_id: string;
  feature_name: string;
  feature_icon: string;
  state: "Registered" | "Opened" | "Focused" | "Closed";
  size: number | null;
}

/** Matches Rust `FeatureMetadata` in `cores/feature-registry/src/lib.rs`. */
export interface FeatureMetadata {
  id: string;
  name: string;
  icon: string;
  capability_file: string;
}

// ── Tauri command signatures (for use with `invoke()`) ────────────

/**
 * Open a panel for a registered feature.
 *
 * Rust: `open_panel(feature_id: String) -> Result<PanelInfo, String>`
 */
export type OpenPanelFn = (featureId: string) => Promise<PanelInfo>;

/**
 * Close a panel by its ID.
 *
 * Rust: `close_panel(panel_id: String) -> Result<(), String>`
 */
export type ClosePanelFn = (panelId: string) => Promise<void>;

/**
 * Focus a panel (bring to front).
 *
 * Rust: `focus_panel(panel_id: String) -> Result<(), String>`
 */
export type FocusPanelFn = (panelId: string) => Promise<void>;

/**
 * Resize a panel to a minimum of 100px.
 *
 * Rust: `resize_panel(panel_id: String, size: f64) -> Result<(), String>`
 */
export type ResizePanelFn = (panelId: string, size: number) => Promise<void>;

/**
 * List all registered features.
 *
 * Rust: `list_features() -> Result<Vec<FeatureMetadata>, String>`
 */
export type ListFeaturesFn = () => Promise<FeatureMetadata[]>;
