//! Chat assistant services (LLM agent + MCP tools).

pub mod agent;
pub mod backends;
pub mod llm;

pub use backends::{build_backend, resolve_api_key};
pub use agent::{models_from_json, run_agent_turn, AgentSseEvent};
