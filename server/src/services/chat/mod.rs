//! Chat assistant services (LLM agent + MCP tools).

pub mod agent;
pub mod anthropic;
pub mod llm;
pub mod openai_compat;

pub use agent::{build_backend, models_from_json, resolve_api_key, run_agent_turn, AgentSseEvent};
