//! Concrete adapters. Each is one module implementing the traits it can and
//! a constructor from a [`vi_core::config::ProviderConfig`].

pub mod anthropic;
pub mod common;
pub mod gemini;
pub mod openai_compat;

pub use anthropic::Anthropic;
pub use gemini::Gemini;
pub use openai_compat::{collect_stream, Collected, OpenAiCompat};
