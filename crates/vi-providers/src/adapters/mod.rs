//! Concrete adapters. Each is one module implementing the traits it can and
//! a constructor from a [`vi_core::config::ProviderConfig`].

pub mod openai_compat;

pub use openai_compat::OpenAiCompat;
