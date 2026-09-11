//! `vi-agent`: the agentic loop from `docs/06-query-and-agents.md`.
//!
//! [`Agent::ask`] streams [`AskEvent`]s: it searches first, lets a
//! tool-using LLM decide whether to read transcript or OCR, look at pixels
//! (`view`), or describe a range with the VLM, and composes an answer with
//! inline citations, all under an [`AskBudget`]. Every tool is read-only
//! against the index and the media; [`tools`] is the same set the MCP server
//! exposes.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod agent;
pub mod citations;
pub mod policy;
pub mod tools;
pub mod view;

pub use agent::{Agent, AskBudget, AskEvent, AskRequest, AskUsage, Collected};
pub use policy::{Policy, RetrievalOnlyPolicy, Step};
pub use tools::{ToolCall, ToolContext, ToolOutput};
pub use view::{render_view, ViewRequest, ViewResult};
