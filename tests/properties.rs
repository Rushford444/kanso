use kanso::{
    model::{Diagnostic, Severity},
    parser, score,
};
use proptest::prelude::*;
use std::path::PathBuf;

proptest! {
    #[test]
    fn arbitrary_markdown_and_yaml_never_panic(source in any::<String>()) {
        let parsed = parser::parse(&source);
        for span in parsed.list_items.iter().chain(parsed.headings.iter().map(|h| &h.span))
            .chain(parsed.links.iter().map(|l| &l.span)).chain(parsed.fences.iter().map(|f| &f.span))
            .chain(parsed.frontmatter.iter()) {
            prop_assert!(span.start <= span.end);
            prop_assert!(source.get(span.start..span.end).is_some());
        }
        let yaml = format!("---\n{source}\n---\n# Document\n");
        let document = kanso::Document { path: PathBuf::from("SKILL.md"), source: yaml };
        let _ = kanso::rules::analyze(&document, &kanso::Config::default(), std::path::Path::new("."))
            .expect("analyze arbitrary frontmatter");
    }

    #[test]
    fn synthetic_github_secrets_never_appear_in_diagnostics(suffix in "[A-Za-z0-9]{36}") {
        let secret = format!("ghp_{suffix}");
        let document = kanso::Document { path: PathBuf::from("AGENTS.md"), source: format!("# Keys\n\nToken: {secret}\n") };
        let diagnostics = kanso::rules::analyze(&document, &kanso::Config::default(), std::path::Path::new("."))
            .expect("analyze synthetic credential");
        prop_assert!(diagnostics.iter().any(|diagnostic| diagnostic.rule_id == "KS007"));
        let serialized = serde_json::to_string(&diagnostics).expect("serialize diagnostics");
        prop_assert!(!serialized.contains(&secret));
    }

    #[test]
    fn output_redaction_covers_credential_families(
        suffix in "[A-Za-z0-9]{36}",
        prefix in prop::sample::select(vec!["ghp_", "sk-", "sk-ant-", "xoxb-", "AKIA"]),
    ) {
        let secret = format!("{prefix}{suffix}");
        let redacted = kanso::rules::redact(&format!("credential: {secret}"));
        prop_assert!(!redacted.contains(&secret));
        prop_assert!(redacted.contains("****"));
    }

    #[test]
    fn pem_bodies_are_not_exposed(body in "[A-Za-z0-9+/]{16,64}") {
        let pem = format!("-----BEGIN PRIVATE KEY-----\n{body}\n-----END PRIVATE KEY-----");
        let redacted = kanso::rules::redact(&pem);
        prop_assert!(!redacted.contains(&body));
        prop_assert!(redacted.starts_with("****"));
    }

    #[test]
    fn score_is_bounded_and_never_increases(entries in prop::collection::vec((0_u8..15, 0_u8..3), 0..300)) {
        let diagnostics: Vec<Diagnostic> = entries.into_iter().map(|(rule, severity)| Diagnostic {
            rule_id: format!("KS{rule:03}"),
            severity: match severity { 0 => Severity::Error, 1 => Severity::Warn, _ => Severity::Info },
            message: String::new(), path: PathBuf::new(), line: 1, column: 1,
        }).collect();
        let result = score::score(&diagnostics);
        prop_assert!(result <= 100);
        if !diagnostics.is_empty() {
            prop_assert!(result <= score::score(&diagnostics[..diagnostics.len() - 1]));
        }
    }
}
