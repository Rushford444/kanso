//! Deterministic capped deduction scoring.
use crate::model::{Diagnostic, Severity};
use std::collections::BTreeMap;
/// Start at 100; deduct 15/5/1 for error/warn/info, capped at 30 per rule.
/// Informational alias records do not affect the score.
#[must_use]
pub fn score(diagnostics: &[Diagnostic]) -> u8 {
    let mut totals = BTreeMap::<&str, u8>::new();
    for d in diagnostics {
        if d.rule_id == "alias" && d.severity == Severity::Info {
            continue;
        }
        let penalty = match d.severity {
            Severity::Error => 15,
            Severity::Warn => 5,
            Severity::Info => 1,
        };
        let total = totals.entry(&d.rule_id).or_default();
        *total = total.saturating_add(penalty).min(30);
    }
    totals
        .values()
        .fold(100_u8, |value, deduction| value.saturating_sub(*deduction))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Diagnostic, Severity};
    use std::path::PathBuf;
    #[test]
    fn scoring_boundaries_and_independent_caps() {
        let diagnostic = |rule: &str, severity| Diagnostic {
            rule_id: rule.to_owned(),
            severity,
            message: String::new(),
            path: PathBuf::new(),
            line: 1,
            column: 1,
        };
        assert_eq!(score(&[]), 100);
        assert_eq!(score(&[diagnostic("A", Severity::Error)]), 85);
        assert_eq!(score(&[diagnostic("A", Severity::Warn)]), 95);
        assert_eq!(score(&[diagnostic("A", Severity::Info)]), 99);
        assert_eq!(score(&[diagnostic("alias", Severity::Info)]), 100);
        assert_eq!(score(&[diagnostic("alias", Severity::Error)]), 85);
        let a = diagnostic("A", Severity::Error);
        let b = diagnostic("B", Severity::Error);
        assert_eq!(
            score(&[a.clone(), a.clone(), a.clone(), b.clone(), b.clone(), b]),
            40
        );
        let all: Vec<_> = (0..10)
            .map(|n| diagnostic(&n.to_string(), Severity::Error))
            .collect();
        assert_eq!(score(&all), 0);
    }
    #[test]
    fn caps_each_rule_and_saturates() {
        let d = Diagnostic {
            rule_id: "KS001".into(),
            severity: Severity::Error,
            message: String::new(),
            path: PathBuf::new(),
            line: 1,
            column: 1,
        };
        assert_eq!(score(&[d.clone(), d.clone(), d]), 70);
    }
}
