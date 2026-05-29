//! AI provider abstraction layer — LLM API clients.

pub mod adapter;
pub mod config;
pub mod encryption;
pub mod storage;

pub use adapter::{
    classify_test_error, extract_response_content, render_request_body, AiProvider,
    AiProviderError, ChatMessage, ProviderTestError, ProviderTestResult,
    TestConnectionErrorKind,
};
pub use config::{default_profiles, ModelInfo, ProviderConfig};
pub use storage::{
    ActiveProvider, NewProviderConfig, ProviderStore, ProviderStoreError, UpdateProviderConfig,
};
