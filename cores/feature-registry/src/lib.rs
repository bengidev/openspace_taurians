//! Feature registry — manages feature metadata, routes commands
//! between features, and tracks panel lifecycle state.

use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::any::{Any, TypeId};

// ── FeatureId ────────────────────────────────────────────────────

/// Unique identifier for a registered feature.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeatureId(String);

impl FeatureId {
    /// Create a new `FeatureId` from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FeatureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for FeatureId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for FeatureId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// ── FeatureMetadata ──────────────────────────────────────────────

/// Metadata describing a registered feature.
///
/// Includes the feature's identity, display name, icon, and the
/// path to its capability declaration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMetadata {
    pub id: FeatureId,
    pub name: String,
    pub icon: String,
    pub capability_file: PathBuf,
}

// ── RegistryError ────────────────────────────────────────────────

/// Errors returned by [`FeatureRegistry`] operations.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("feature with id '{0}' is already registered")]
    DuplicateRegistration(FeatureId),

    #[error("feature with id '{0}' not found")]
    NotFound(FeatureId),
}

// ── FeatureRegistry ──────────────────────────────────────────────

/// Central registry of feature metadata.
///
/// Stores metadata for all registered features and provides
/// lookup by `FeatureId`. Rejects duplicate registrations.
pub struct FeatureRegistry {
    features: HashMap<FeatureId, FeatureMetadata>,
}

impl FeatureRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            features: HashMap::new(),
        }
    }

    /// Register a feature. Returns [`RegistryError::DuplicateRegistration`]
    /// if a feature with the same `FeatureId` already exists.
    pub fn register(&mut self, meta: FeatureMetadata) -> Result<(), RegistryError> {
        if self.features.contains_key(&meta.id) {
            return Err(RegistryError::DuplicateRegistration(meta.id));
        }
        self.features.insert(meta.id.clone(), meta);
        Ok(())
    }

    /// Remove a feature by id. Returns [`RegistryError::NotFound`]
    /// if no feature with the given id is registered.
    pub fn unregister(&mut self, id: &FeatureId) -> Result<(), RegistryError> {
        self.features
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| RegistryError::NotFound(id.clone()))
    }

    /// Look up a feature by id. Returns `None` if not found.
    pub fn get(&self, id: &FeatureId) -> Option<&FeatureMetadata> {
        self.features.get(id)
    }

    /// Return all registered features.
    pub fn list(&self) -> Vec<&FeatureMetadata> {
        self.features.values().collect()
    }
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── PanelLifecycle ───────────────────────────────────────────────

/// States in the panel lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelState {
    Registered,
    Opened,
    Focused,
    Closed,
}

/// Events that drive panel state transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelEvent {
    Open,
    Focus,
    Close,
}

/// Error returned when an invalid panel state transition is
/// attempted.
#[derive(Debug, Error)]
#[error("invalid transition from {from:?} via event {event:?}")]
pub struct TransitionError {
    pub from: PanelState,
    pub event: PanelEvent,
}

/// Tracks the lifecycle state of a feature panel.
///
/// State machine: `Registered → Opened → Focused → Closed`.
/// Invalid transitions (e.g. `Closed → Focused`) return
/// [`TransitionError`].
pub struct PanelLifecycle {
    state: PanelState,
}

impl PanelLifecycle {
    /// Create a new lifecycle tracker starting at
    /// [`PanelState::Registered`].
    pub fn new() -> Self {
        Self {
            state: PanelState::Registered,
        }
    }

    /// Return the current state.
    pub fn state(&self) -> PanelState {
        self.state
    }

    /// Attempt to transition to a new state based on the given event.
    ///
    /// Returns the new state on success, or
    /// [`TransitionError`] if the transition is not valid.
    pub fn transition(&mut self, event: PanelEvent) -> Result<PanelState, TransitionError> {
        let next = match (self.state, event) {
            (PanelState::Registered, PanelEvent::Open) => PanelState::Opened,
            (PanelState::Opened, PanelEvent::Focus) => PanelState::Focused,
            (PanelState::Focused, PanelEvent::Close) => PanelState::Closed,
            _ => {
                return Err(TransitionError {
                    from: self.state,
                    event,
                })
            }
        };
        self.state = next;
        Ok(self.state)
    }
}

impl Default for PanelLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

// ── CommandBus ───────────────────────────────────────────────────

/// Errors returned by [`CommandBus`] operations.
#[derive(Debug, Error)]
pub enum BusError {
    #[error("no handler registered for request type '{request_type}'")]
    NoHandler { request_type: &'static str },

    #[error("type mismatch for request '{request_type}' → response '{response_type}'")]
    TypeMismatch {
        request_type: &'static str,
        response_type: &'static str,
    },
}

/// Type-erased handler that accepts a boxed request and returns a
/// boxed response.
trait ErasedHandler: Send + Sync {
    fn handle(&self, request: Box<dyn Any>) -> Result<Box<dyn Any>, BusError>;
}

struct HandlerWrapper<Req, Res, F> {
    handler: F,
    _phantom: PhantomData<(Req, Res)>,
}

impl<Req, Res, F> ErasedHandler for HandlerWrapper<Req, Res, F>
where
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
    F: Fn(Req) -> Res + Send + Sync,
{
    fn handle(&self, request: Box<dyn Any>) -> Result<Box<dyn Any>, BusError> {
        let request = request
            .downcast::<Req>()
            .map_err(|_| BusError::TypeMismatch {
                request_type: std::any::type_name::<Req>(),
                response_type: std::any::type_name::<Res>(),
            })?;
        let response = (self.handler)(*request);
        Ok(Box::new(response))
    }
}

/// Mediator-pattern typed request/response routing between features.
///
/// Features register handlers for specific request types, and the bus
/// routes incoming requests to the matching handler. Type-safe —
/// mismatched request/response types are caught at compile time.
///
/// v1 is synchronous; the design accommodates future async extension
/// without breaking the type signature.
pub struct CommandBus {
    handlers: HashMap<TypeId, Box<dyn ErasedHandler>>,
}

impl CommandBus {
    /// Create an empty command bus.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for requests of type `Req` that produces
    /// responses of type `Res`.
    ///
    /// The handler is type-checked at registration time — the
    /// request and response types must match what callers of
    /// [`send`](Self::send) will use.
    pub fn register<Req, Res, F>(&mut self, handler: F)
    where
        Req: Send + Sync + 'static,
        Res: Send + Sync + 'static,
        F: Fn(Req) -> Res + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<Req>();
        let wrapper = HandlerWrapper {
            handler,
            _phantom: PhantomData,
        };
        self.handlers.insert(type_id, Box::new(wrapper));
    }

    /// Send a request and receive a typed response.
    ///
    /// Returns [`BusError::NoHandler`] if no handler is registered
    /// for the request type.
    pub fn send<Req, Res>(&self, request: Req) -> Result<Res, BusError>
    where
        Req: 'static,
        Res: 'static,
    {
        let type_id = TypeId::of::<Req>();
        let handler = self
            .handlers
            .get(&type_id)
            .ok_or_else(|| BusError::NoHandler {
                request_type: std::any::type_name::<Req>(),
            })?;

        let response = handler.handle(Box::new(request))?;
        response
            .downcast::<Res>()
            .map(|b| *b)
            .map_err(|_| BusError::TypeMismatch {
                request_type: std::any::type_name::<Req>(),
                response_type: std::any::type_name::<Res>(),
            })
    }
}

impl Default for CommandBus {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;

    // ── Send + Sync guarantees ────────────────────────────────

    #[test]
    fn feature_metadata_is_send_sync() {
        assert_impl_all!(FeatureMetadata: Send, Sync);
    }

    #[test]
    fn feature_registry_is_send_sync() {
        assert_impl_all!(FeatureRegistry: Send, Sync);
    }

    #[test]
    fn panel_lifecycle_is_send_sync() {
        assert_impl_all!(PanelLifecycle: Send, Sync);
    }

    #[test]
    fn command_bus_is_send_sync() {
        assert_impl_all!(CommandBus: Send, Sync);
    }

    // ── FeatureRegistry helpers ───────────────────────────────

    fn make_meta(id: &str) -> FeatureMetadata {
        FeatureMetadata {
            id: FeatureId::new(id),
            name: format!("Feature {id}"),
            icon: "icon.svg".into(),
            capability_file: PathBuf::from(format!("{id}.toml")),
        }
    }

    // ── FeatureRegistry tests ─────────────────────────────────

    #[test]
    fn register_and_list() {
        let mut reg = FeatureRegistry::new();
        let meta = make_meta("editor");
        reg.register(meta).unwrap();
        assert_eq!(reg.list().len(), 1);
        assert!(reg.get(&FeatureId::new("editor")).is_some());
    }

    #[test]
    fn register_duplicate_rejected() {
        let mut reg = FeatureRegistry::new();
        reg.register(make_meta("terminal")).unwrap();
        let result = reg.register(make_meta("terminal"));
        assert!(matches!(
            result,
            Err(RegistryError::DuplicateRegistration(_))
        ));
    }

    #[test]
    fn unregister_existing() {
        let mut reg = FeatureRegistry::new();
        reg.register(make_meta("chat")).unwrap();
        assert!(reg.unregister(&FeatureId::new("chat")).is_ok());
        assert!(reg.get(&FeatureId::new("chat")).is_none());
    }

    #[test]
    fn unregister_nonexistent() {
        let mut reg = FeatureRegistry::new();
        let result = reg.unregister(&FeatureId::new("nope"));
        assert!(matches!(result, Err(RegistryError::NotFound(_))));
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let reg = FeatureRegistry::new();
        assert!(reg.get(&FeatureId::new("missing")).is_none());
    }

    #[test]
    fn registry_new_is_empty() {
        let reg = FeatureRegistry::new();
        assert!(reg.list().is_empty());
    }

    // ── PanelLifecycle tests ──────────────────────────────────

    #[test]
    fn panel_lifecycle_happy_path() {
        let mut panel = PanelLifecycle::new();
        assert_eq!(panel.state(), PanelState::Registered);

        let state = panel.transition(PanelEvent::Open).unwrap();
        assert_eq!(state, PanelState::Opened);
        assert_eq!(panel.state(), PanelState::Opened);

        let state = panel.transition(PanelEvent::Focus).unwrap();
        assert_eq!(state, PanelState::Focused);

        let state = panel.transition(PanelEvent::Close).unwrap();
        assert_eq!(state, PanelState::Closed);
    }

    #[test]
    fn invalid_transition_closed_to_focused() {
        let mut panel = PanelLifecycle::new();
        panel.transition(PanelEvent::Open).unwrap();
        panel.transition(PanelEvent::Focus).unwrap();
        panel.transition(PanelEvent::Close).unwrap();

        let result = panel.transition(PanelEvent::Focus);
        let err = result.unwrap_err();
        assert!(matches!(err.from, PanelState::Closed));
        assert!(matches!(err.event, PanelEvent::Focus));
        // state should not have changed
        assert_eq!(panel.state(), PanelState::Closed);
    }

    #[test]
    fn invalid_transition_registered_to_close() {
        let mut panel = PanelLifecycle::new();
        let result = panel.transition(PanelEvent::Close);
        let err = result.unwrap_err();
        assert!(matches!(err.from, PanelState::Registered));
        assert!(matches!(err.event, PanelEvent::Close));
    }

    #[test]
    fn invalid_transition_opened_to_close() {
        let mut panel = PanelLifecycle::new();
        panel.transition(PanelEvent::Open).unwrap();

        let result = panel.transition(PanelEvent::Close);
        let err = result.unwrap_err();
        assert!(matches!(err.from, PanelState::Opened));
        assert!(matches!(err.event, PanelEvent::Close));
    }

    #[test]
    fn invalid_transition_registered_to_focus() {
        let mut panel = PanelLifecycle::new();
        let result = panel.transition(PanelEvent::Focus);
        assert!(result.is_err());
    }

    #[test]
    fn lifecycle_new_starts_at_registered() {
        let panel = PanelLifecycle::new();
        assert_eq!(panel.state(), PanelState::Registered);
    }

    #[test]
    fn lifecycle_default_starts_at_registered() {
        let panel = PanelLifecycle::default();
        assert_eq!(panel.state(), PanelState::Registered);
    }

    // ── FeatureMetadata serde round-trip ──────────────────────

    #[test]
    fn feature_metadata_serde_roundtrip() {
        let meta = FeatureMetadata {
            id: FeatureId::new("git-lens"),
            name: "Git Lens".into(),
            icon: "git-lens.svg".into(),
            capability_file: PathBuf::from("capabilities/git-lens.json"),
        };

        let json = serde_json::to_string(&meta).unwrap();
        let roundtripped: FeatureMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtripped.id, meta.id);
        assert_eq!(roundtripped.name, meta.name);
        assert_eq!(roundtripped.icon, meta.icon);
        assert_eq!(roundtripped.capability_file, meta.capability_file);
    }

    // ── CommandBus tests ──────────────────────────────────────

    #[derive(Debug, PartialEq)]
    struct GetName {
        id: String,
    }

    #[derive(Debug, PartialEq)]
    struct NameResult {
        name: String,
    }

    #[test]
    fn command_bus_register_and_send() {
        let mut bus = CommandBus::new();

        bus.register(|req: GetName| -> NameResult {
            NameResult {
                name: format!("name-of-{}", req.id),
            }
        });

        let result: NameResult = bus.send(GetName { id: "42".into() }).unwrap();
        assert_eq!(
            result,
            NameResult {
                name: "name-of-42".into()
            }
        );
    }

    #[test]
    fn command_bus_no_handler() {
        let bus = CommandBus::new();
        let result: Result<String, _> = bus.send::<String, String>("hello".into());
        assert!(matches!(result, Err(BusError::NoHandler { .. })));
    }

    #[test]
    fn command_bus_multiple_handlers() {
        let mut bus = CommandBus::new();

        bus.register(|req: GetName| -> NameResult {
            NameResult {
                name: format!("name-{}", req.id),
            }
        });

        #[derive(Debug)]
        struct GetVersion;
        bus.register(|_: GetVersion| -> String { "1.0.0".into() });

        let name: NameResult = bus.send(GetName { id: "x".into() }).unwrap();
        assert_eq!(name.name, "name-x");

        let version: String = bus.send(GetVersion).unwrap();
        assert_eq!(version, "1.0.0");
    }
}
