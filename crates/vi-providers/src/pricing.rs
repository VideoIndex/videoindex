//! Default price tables per model, USD. These are the adapters' defaults
//! and are overridden by `pricing` in the provider's config table; treat
//! them as estimates to be checked against the vendor's current price
//! list. Unknown models cost zero, which is right for local servers.

use vi_core::config::Pricing;

fn per_mtok(input: f64, output: f64) -> Pricing {
    Pricing {
        input_per_mtok: input,
        output_per_mtok: output,
        per_image: 0.0,
        per_media_second: 0.0,
        per_call: 0.0,
    }
}

/// Best-known default prices for a model name (prefix match on the family).
pub fn default_pricing(model: &str) -> Pricing {
    let m = model.to_ascii_lowercase();
    // Anthropic
    if m.contains("claude-opus") {
        return per_mtok(15.0, 75.0);
    }
    if m.contains("claude-sonnet") {
        return per_mtok(3.0, 15.0);
    }
    if m.contains("claude-haiku") {
        return per_mtok(1.0, 5.0);
    }
    if m.contains("claude-fable") || m.contains("claude-mythos") {
        return per_mtok(15.0, 75.0);
    }
    // Google
    if m.contains("gemini-2.5-pro") || m.contains("gemini-3-pro") {
        return per_mtok(1.25, 10.0);
    }
    if m.contains("gemini-2.5-flash-lite") {
        return per_mtok(0.10, 0.40);
    }
    // Gemini 3.x Flash-Lite ($0.30 / $2.50) and Flash ($0.75 / $3.75 through 2026-12-31).
    if m.contains("gemini-3") && m.contains("flash-lite") {
        return per_mtok(0.30, 2.50);
    }
    if m.contains("gemini-3") && m.contains("flash") {
        return per_mtok(0.75, 3.75);
    }
    if m.contains("gemini-2.5-flash") {
        return per_mtok(0.30, 2.50);
    }
    // OpenAI
    if m.contains("gpt-4o-mini") || m.contains("gpt-4.1-mini") {
        return per_mtok(0.15, 0.60);
    }
    if m.contains("gpt-4o") || m.contains("gpt-4.1") {
        return per_mtok(2.50, 10.0);
    }
    if m.starts_with("o3") || m.starts_with("o4") {
        return per_mtok(2.0, 8.0);
    }
    if m.contains("whisper") {
        return Pricing {
            per_media_second: 0.006 / 60.0,
            ..Pricing::default()
        };
    }
    if m.contains("text-embedding-3-small") {
        return per_mtok(0.02, 0.0);
    }
    if m.contains("text-embedding-3-large") {
        return per_mtok(0.13, 0.0);
    }
    Pricing::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_and_unknown_models() {
        assert!(default_pricing("claude-sonnet-5").input_per_mtok > 0.0);
        assert!(default_pricing("gemini-2.5-flash").output_per_mtok > 0.0);
        assert!((default_pricing("gemini-3.8-flash").input_per_mtok - 0.75).abs() < 1e-9);
        assert!((default_pricing("gemini-3.5-flash-lite").input_per_mtok - 0.30).abs() < 1e-9);
        assert_eq!(
            default_pricing("Qwen/Qwen2.5-VL-32B-Instruct"),
            Pricing::default()
        );
        assert!(default_pricing("whisper-large-v3").per_media_second > 0.0);
    }
}
