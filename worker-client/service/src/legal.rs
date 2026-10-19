//! Legal texts embedded at compile time and the worker's terms acceptance (LEG-04..07).
//!
//! The texts come only from `docs/legal/` (LEG-04) and are shown in full by the window. Their
//! version and hash are computed with `telemetry_crypto_core::legal`, the same functions the API
//! uses, so the acceptance evidence matches byte for byte. `build.rs` refuses a `release` build
//! while these texts still contain owner placeholders (LEG-06).

use agent_ipc::{LegalDocument, LegalDocumentKind};
use telemetry_crypto_core::legal::{document_hash, parse_version};

/// Worker terms (`docs/legal/terminos-trabajador.md`).
pub const WORKER_TERMS: &str = include_str!("../../../docs/legal/terminos-trabajador.md");
/// Privacy policy (`docs/legal/politica-privacidad.md`).
pub const PRIVACY_POLICY: &str = include_str!("../../../docs/legal/politica-privacidad.md");

/// Wire name of the worker terms in `POST /workers/me/terms-acceptance`.
pub const WORKER_TERMS_DOCUMENT: &str = "WORKER_TERMS";

/// Version and hash of one embedded document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentId {
    pub version: String,
    pub hash: String,
}

fn identify(text: &str) -> DocumentId {
    DocumentId {
        // A missing header would be a broken build input; an empty version makes the server
        // answer `409 TERMS_VERSION_MISMATCH` instead of accepting an unidentified text.
        version: parse_version(text).unwrap_or_default(),
        hash: document_hash(text),
    }
}

/// `{version, hash}` of the embedded worker terms.
pub fn worker_terms_id() -> DocumentId {
    identify(WORKER_TERMS)
}

/// `{version, hash}` of the embedded privacy policy.
pub fn privacy_policy_id() -> DocumentId {
    identify(PRIVACY_POLICY)
}

/// Embedded documents for the `LEGAL_DOCUMENTS` IPC op (terms first, then privacy, as T1 shows).
pub fn documents() -> Vec<LegalDocument> {
    let terms = worker_terms_id();
    let privacy = privacy_policy_id();
    vec![
        LegalDocument {
            document: LegalDocumentKind::WorkerTerms,
            version: terms.version,
            hash: terms.hash,
            text: WORKER_TERMS.to_string(),
        },
        LegalDocument {
            document: LegalDocumentKind::Privacy,
            version: privacy.version,
            hash: privacy.hash,
            text: PRIVACY_POLICY.to_string(),
        },
    ]
}

/// True when the values the window sent are exactly the embedded ones (LEG-04): the worker
/// accepts what this binary displays, nothing else.
pub fn matches_embedded(
    version: &str,
    hash: &str,
    privacy_version: &str,
    privacy_hash: &str,
) -> bool {
    let terms = worker_terms_id();
    let privacy = privacy_policy_id();
    terms.version == version
        && terms.hash == hash
        && privacy.version == privacy_version
        && privacy.hash == privacy_hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use telemetry_crypto_core::legal::find_placeholders;

    // The same file `build.rs` includes: the build-time copy of the LEG-06 rule.
    mod build_rule {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/build_support/placeholders.rs"
        ));
    }

    #[test]
    fn test_legal_embedded_documents_have_version_and_hash() {
        let docs = documents();
        assert_eq!(docs.len(), 2);
        for doc in &docs {
            assert_eq!(doc.version, "2026-10-11");
            assert_eq!(doc.hash.len(), 64);
            assert_eq!(doc.hash, document_hash(&doc.text));
        }
        assert_eq!(docs[0].document, LegalDocumentKind::WorkerTerms);
        assert_eq!(docs[0].text, WORKER_TERMS);
        assert_eq!(docs[1].text, PRIVACY_POLICY);
    }

    #[test]
    fn test_legal_matches_embedded_requires_all_four_values() {
        let t = worker_terms_id();
        let p = privacy_policy_id();
        assert!(matches_embedded(&t.version, &t.hash, &p.version, &p.hash));
        assert!(!matches_embedded(
            "2020-01-01",
            &t.hash,
            &p.version,
            &p.hash
        ));
        assert!(!matches_embedded(&t.version, &p.hash, &p.version, &p.hash));
        assert!(!matches_embedded(&t.version, &t.hash, &p.version, &t.hash));
    }

    #[test]
    fn test_legal_build_rs_placeholder_rule_matches_crypto_core_on_real_files() {
        // LEG-06: build.rs cannot depend on the crate, so it carries a minimal copy of the rule.
        // Both must find exactly the same placeholders in the real texts.
        for text in [WORKER_TERMS, PRIVACY_POLICY] {
            assert_eq!(build_rule::find_placeholders(text), find_placeholders(text));
        }
        let sample = "[ ] [x] [link](u) [RUT] [N] [MIENTRAS NO EXISTA:] [minúscula] […]";
        assert_eq!(
            build_rule::find_placeholders(sample),
            find_placeholders(sample)
        );
        // Today the texts still carry owner placeholders, so a release build must fail.
        assert!(!build_rule::find_placeholders(WORKER_TERMS).is_empty());
    }
}
