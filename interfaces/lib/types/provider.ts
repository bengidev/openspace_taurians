export interface ModelInfo {
  id: string;
  name: string;
  context_window: number;
}

export interface Provider {
  id: number;
  name: string;
  base_url: string;
  api_key_redacted: string;
  has_api_key: boolean;
  auth_header_name: string;
  auth_header_value_prefix: string;
  models: ModelInfo[];
  request_body_template: Record<string, unknown>;
  response_path: string;
}

export interface ProviderCreate {
  name: string;
  base_url: string;
  api_key?: string;
  auth_header_name?: string;
  auth_header_value_prefix?: string;
  models: ModelInfo[];
  request_body_template: Record<string, unknown>;
  response_path: string;
}

export interface ProviderUpdate {
  id: number;
  name: string;
  base_url: string;
  api_key?: string;
  auth_header_name?: string;
  auth_header_value_prefix?: string;
  models: ModelInfo[];
  request_body_template: Record<string, unknown>;
  response_path: string;
}

export type TestConnectionErrorKind =
  | "auth"
  | "network"
  | "invalid_config"
  | "http_status"
  | "malformed_response"
  | "unknown";

export interface ProviderTestError {
  kind: TestConnectionErrorKind;
  message: string;
}

export interface ProviderTestResult {
  success: boolean;
  error?: ProviderTestError;
}

export interface ActiveProvider {
  provider_id: number;
  model: string;
}

// ── Chat types (sent to provider_chat_stream / chat_send_stream) ──

export interface ChatMessage {
  role: string;
  content: string;
}
