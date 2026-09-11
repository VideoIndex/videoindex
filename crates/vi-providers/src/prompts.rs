//! Versioned prompt assets. Each prompt is a Markdown file under
//! `prompts/`, compiled in, hashed, and looked up by name; the hash goes
//! into cache keys and `Provenance` rows. Overrides registered at runtime
//! (from Python or JS, later) replace the text and change the hash.

use std::collections::BTreeMap;
use std::sync::{OnceLock, RwLock};

/// A prompt with its content hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// Name, e.g. `agent_system`.
    pub name: &'static str,
    /// Text.
    pub text: String,
    /// blake3 of the text, first 16 hex characters.
    pub hash: String,
}

const BUILTIN: &[(&str, &str)] = &[
    ("agent_system", include_str!("../prompts/agent_system.md")),
    ("vlm_describe", include_str!("../prompts/vlm_describe.md")),
    (
        "entities_events",
        include_str!("../prompts/entities_events.md"),
    ),
];

fn overrides() -> &'static RwLock<BTreeMap<String, String>> {
    static O: OnceLock<RwLock<BTreeMap<String, String>>> = OnceLock::new();
    O.get_or_init(|| RwLock::new(BTreeMap::new()))
}

/// Fetch a prompt by name, with any override applied.
pub fn get(name: &str) -> Option<Prompt> {
    let (n, builtin) = BUILTIN.iter().find(|(n, _)| *n == name)?;
    let text = overrides()
        .read()
        .ok()
        .and_then(|o| o.get(name).cloned())
        .unwrap_or_else(|| builtin.to_string());
    let hash = crate::cost::prompt_hash(&text);
    Some(Prompt {
        name: n,
        text,
        hash,
    })
}

/// Replace a prompt's text for this process.
pub fn override_prompt(name: &str, text: impl Into<String>) {
    if let Ok(mut o) = overrides().write() {
        o.insert(name.to_string(), text.into());
    }
}

/// Names of all built-in prompts.
pub fn names() -> Vec<&'static str> {
    BUILTIN.iter().map(|(n, _)| *n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_prompts_have_hashes_and_overrides_change_them() {
        let p = get("agent_system").unwrap();
        assert!(p.text.contains("cite:"));
        assert_eq!(p.hash.len(), 16);
        override_prompt("agent_system", "custom");
        let q = get("agent_system").unwrap();
        assert_eq!(q.text, "custom");
        assert_ne!(q.hash, p.hash);
        assert!(get("nope").is_none());
        assert!(names().contains(&"vlm_describe"));
    }
}
