//! Product client names (ADR-011 §1): persist `claude|cursor|opencode|codex` only.

/// Map a binary / alias / product id to the persisted product name.
/// Unknown ids (`shell`, `pi`, …) pass through unchanged.
pub fn normalize_client_name(raw: &str) -> &str {
    match raw {
        "claude" | "claude-code" => "claude",
        "cursor" | "cursor-agent" | "agent" => "cursor",
        "opencode" => "opencode",
        "codex" => "codex",
        other => other,
    }
}

/// True when `name` is one of the four product clients (after normalisation).
pub fn is_product_client(name: &str) -> bool {
    matches!(
        normalize_client_name(name),
        "claude" | "cursor" | "opencode" | "codex"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_map_to_product_names() {
        let cases = [
            ("claude", "claude"),
            ("claude-code", "claude"),
            ("cursor", "cursor"),
            ("cursor-agent", "cursor"),
            ("agent", "cursor"),
            ("opencode", "opencode"),
            ("codex", "codex"),
            ("shell", "shell"),
            ("pi", "pi"),
        ];
        for (raw, want) in cases {
            assert_eq!(normalize_client_name(raw), want, "raw={raw}");
        }
        assert!(is_product_client("agent"));
        assert!(!is_product_client("shell"));
    }
}
