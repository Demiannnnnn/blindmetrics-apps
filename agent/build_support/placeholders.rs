// Minimal copy of `telemetry_crypto_core::legal::find_placeholders` for `build.rs` (LEG-06):
// the build script cannot depend on the crate it builds. `agent/src/legal.rs` includes this same
// file in a test that compares both implementations on the real legal texts.

/// Bracketed owner placeholders (`[RUT]`, `[RAZÓN SOCIAL…]`, `[MIENTRAS …]`), excluding Markdown
/// links (`[text](url)`) and checkboxes (`[ ]`, `[x]`, `[X]`).
pub fn find_placeholders(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after_open = &rest[open + 1..];
        let close = after_open.find([']', '[', '\n']);
        match close {
            Some(end) if after_open[end..].starts_with(']') => {
                let inner = &after_open[..end];
                let tail = &after_open[end + 1..];
                let trimmed = inner.trim();
                let is_link = tail.starts_with('(');
                let is_checkbox = trimmed.is_empty() || trimmed == "x" || trimmed == "X";
                let starts_upper = inner.chars().next().is_some_and(char::is_uppercase);
                if !is_link && !is_checkbox && starts_upper {
                    found.push(format!("[{inner}]"));
                }
                rest = tail;
            }
            _ => rest = after_open,
        }
    }
    found
}
