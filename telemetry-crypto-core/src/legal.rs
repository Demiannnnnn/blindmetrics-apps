//! Legal document hashing, version parsing, and placeholder detection (LEG-04, LEG-06).
//!
//! Enforces exact versioning, tamper-evident document hashing, and release-blocking
//! placeholder validation for legal contracts and policies.

use sha2::{Digest, Sha256};

/// Computes the canonical SHA-256 hash (in lowercase hex) of a legal document's UTF-8 bytes (LEG-04).
#[must_use]
pub fn document_hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    hex::encode(digest)
}

/// Parses the document version from the header line `**Versión:** AAAA-MM-DD` (LEG-04).
///
/// Any text after ` · ` (such as status/draft notes) is ignored.
/// Returns `Some(version)` if found and validated as a valid `YYYY-MM-DD` date, or `None`.
#[must_use]
pub fn parse_version(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(pos) = line.find("**Versión:**") {
            let after = &line[pos + "**Versión:**".len()..];
            if let Some(part) = after.split('·').next() {
                let candidate = part.trim();
                if let Some(first_word) = candidate.split_whitespace().next() {
                    if first_word.len() == 10
                        && chrono::NaiveDate::parse_from_str(first_word, "%Y-%m-%d").is_ok()
                    {
                        return Some(first_word.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Detects unpopulated bracket placeholders in legal texts (LEG-06).
///
/// Matches bracketed terms that start with an uppercase letter or `MIENTRAS`
/// (e.g. `[RAZÓN SOCIAL DEL PROVEEDOR]`, `[RUT]`, `[N]`, `[MIENTRAS NO EXISTA AUTORIZACIÓN:]`).
/// Excludes Markdown links (`[texto](url)`) and checkboxes (`[ ]`, `[x]`, `[X]`).
#[must_use]
pub fn find_placeholders(text: &str) -> Vec<String> {
    let mut results = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'[' {
            let start = i;
            let mut j = i + 1;
            let mut found_close = false;
            while j < len {
                if bytes[j] == b'\n' || bytes[j] == b'[' {
                    break;
                }
                if bytes[j] == b']' {
                    found_close = true;
                    break;
                }
                j += 1;
            }

            if found_close {
                let end = j;
                let inner = &text[start + 1..end];
                let after = &text[end + 1..];

                // 1. Exclude markdown links [text](url)
                if after.starts_with('(') {
                    i = end + 1;
                    continue;
                }

                // 2. Exclude checkboxes [ ], [x], [X]
                let trimmed_inner = inner.trim();
                if trimmed_inner.is_empty()
                    || trimmed_inner == "x"
                    || trimmed_inner == "X"
                    || inner == " "
                {
                    i = end + 1;
                    continue;
                }

                // 3. Must start with uppercase or "MIENTRAS"
                let starts_with_uppercase = inner
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false);
                let starts_with_mientras = inner.starts_with("MIENTRAS");

                if starts_with_uppercase || starts_with_mientras {
                    results.push(format!("[{}]", inner));
                }

                i = end + 1;
                continue;
            }
        }
        i += 1;
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKER_TERMS: &str = include_str!("../../docs/legal/terminos-trabajador.md");
    const EMPLOYER_TERMS: &str = include_str!("../../docs/legal/terminos-empleador.md");
    const PRIVACY_POLICY: &str = include_str!("../../docs/legal/politica-privacidad.md");
    const LEGAL_README: &str = include_str!("../../docs/legal/README.md");

    #[test]
    fn test_legal_document_hash_is_sha256() {
        let text = "Hello legal world!";
        let hash = document_hash(text);
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

        let expected = hex::encode(Sha256::digest(text.as_bytes()));
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_legal_parse_version_real_docs() {
        assert_eq!(parse_version(WORKER_TERMS), Some("2026-10-11".to_string()));
        assert_eq!(
            parse_version(EMPLOYER_TERMS),
            Some("2026-10-11".to_string())
        );
        assert_eq!(
            parse_version(PRIVACY_POLICY),
            Some("2026-10-11".to_string())
        );

        // Text without version
        assert_eq!(parse_version("No version header here"), None);

        // Invalid date format
        assert_eq!(parse_version("**Versión:** 2026-13-45 · borrador"), None);
    }

    #[test]
    fn test_legal_find_placeholders_ignores_links_and_checkboxes() {
        let sample = "Item 1: [ ] empty checkbox, [x] checked, [X] capital checked.\n\
                      Link: [Política de privacidad](politica-privacidad.md).\n\
                      Placeholders: [RAZÓN SOCIAL DEL PROVEEDOR] and [RUT], also [N] and [MIENTRAS NO EXISTA AUTORIZACIÓN:].\n\
                      Not a placeholder: […], [`code_file.md`].";

        let placeholders = find_placeholders(sample);
        assert_eq!(
            placeholders,
            vec![
                "[RAZÓN SOCIAL DEL PROVEEDOR]",
                "[RUT]",
                "[N]",
                "[MIENTRAS NO EXISTA AUTORIZACIÓN:]"
            ]
        );

        // Verify the 4 real legal files contain unpopulated placeholders today
        assert!(!find_placeholders(WORKER_TERMS).is_empty());
        assert!(!find_placeholders(EMPLOYER_TERMS).is_empty());
        assert!(!find_placeholders(PRIVACY_POLICY).is_empty());
        assert!(!find_placeholders(LEGAL_README).is_empty());
    }
}
