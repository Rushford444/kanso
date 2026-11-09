use std::ops::Range;

fn entropy(token: &str) -> f64 {
    let mut counts = [0_usize; 256];
    // A bounded sample avoids precision loss for arbitrarily long untrusted input.
    for byte in token.bytes().take(4096) {
        counts[usize::from(byte)] += 1;
    }
    let length = f64::from(u32::try_from(token.len().min(4096)).unwrap_or(4096));
    counts
        .into_iter()
        .filter(|&count| count > 0)
        .map(|count| {
            let p = f64::from(u32::try_from(count).unwrap_or(4096)) / length;
            -p * p.log2()
        })
        .sum()
}

fn known_secret_token(token: &str) -> bool {
    (token.starts_with("AKIA") || token.starts_with("ASIA")) && token.len() >= 20
        || [
            "ghp_",
            "gho_",
            "ghu_",
            "ghs_",
            "ghr_",
            "github_pat_",
            "xoxb-",
            "xoxp-",
            "xoxa-",
            "xoxr-",
            "xoxs-",
        ]
        .iter()
        .any(|prefix| token.starts_with(prefix) && token.len() >= prefix.len() + 8)
        || (token.starts_with("sk-") && token.len() >= 20)
}

fn high_entropy_token(token: &str) -> bool {
    token.len() >= 32
        && token.bytes().any(|b| b.is_ascii_digit())
        && token.bytes().any(|b| b.is_ascii_alphabetic())
        && entropy(token) >= 4.0
}

fn secret_token(token: &str) -> bool {
    known_secret_token(token) || high_entropy_token(token)
}

pub(crate) fn path_has_secret(path: &str) -> bool {
    path.split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        .any(|token| {
            if token.starts_with("AKIA") || token.starts_with("ASIA") {
                return token.len() == 20
                    && token[4..]
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
            }
            if let Some(rest) = token.strip_prefix("sk-") {
                let rest = rest
                    .strip_prefix("proj-")
                    .or_else(|| rest.strip_prefix("ant-"))
                    .unwrap_or(rest);
                return token.len() >= 20 && rest.bytes().all(|b| b.is_ascii_alphanumeric());
            }
            known_secret_token(token)
        })
        || path
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(high_entropy_token)
}

fn context_before(text: &str, position: usize) -> String {
    let mut start = position.saturating_sub(128);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..position]
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .to_string()
}

fn has_keyword(context: &str, keyword: &str) -> bool {
    context.split(|c: char| !c.is_alphanumeric()).any(|word| {
        let mut start = 0;
        let mut chars = word.char_indices().peekable();
        let mut previous = None;
        while let Some((index, current)) = chars.next() {
            let acronym_boundary = previous.is_some_and(|c: char| c.is_uppercase())
                && current.is_uppercase()
                && chars.peek().is_some_and(|(_, next)| next.is_lowercase());
            let camel_boundary =
                previous.is_some_and(|c: char| c.is_lowercase()) && current.is_uppercase();
            if acronym_boundary || camel_boundary {
                if word[start..index].eq_ignore_ascii_case(keyword) {
                    return true;
                }
                start = index;
            }
            previous = Some(current);
        }
        if word[start..].eq_ignore_ascii_case(keyword) {
            return true;
        }
        let lower = word.to_ascii_lowercase();
        match keyword {
            "key" => {
                ["apikey", "secretkey", "privatekey", "awssecretkey"].contains(&lower.as_str())
            }
            "token" => ["accesstoken", "authtoken"].contains(&lower.as_str()),
            "secret" => ["secretkey", "clientsecret", "awssecretkey"].contains(&lower.as_str()),
            "aws" => lower == "awssecretkey",
            _ => false,
        }
    })
}

fn contextual_token(token: &str, context: &str) -> bool {
    let is_hex = token.len() >= 32 && token.bytes().all(|b| b.is_ascii_hexdigit());
    if is_hex {
        // A token budget is a measurement, not a credential label.
        let mut context = context.to_string();
        let lower_context = context.to_ascii_lowercase();
        let mut offset = 0;
        while let Some(found) = lower_context[offset..].find("token budget") {
            let start = offset + found;
            let end = start + "token budget".len();
            let before = context[..start].chars().next_back();
            let after = context[end..].chars().next();
            if !before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '-'))
                && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
            {
                context.replace_range(start..start + "token".len(), "     ");
            }
            offset = end;
        }
        ["key", "token", "secret"]
            .iter()
            .any(|word| has_keyword(&context, word))
    } else {
        // CI cache-key examples commonly contain a hexadecimal digest.
        let lower_context = context.to_ascii_lowercase();
        let prefix = lower_context.trim_end_matches([' ', ':', '`']);
        let cache_example = prefix
            .strip_suffix("cache key format")
            .is_some_and(|preceding| {
                !["key", "token", "secret"]
                    .iter()
                    .any(|word| has_keyword(preceding, word))
            })
            && token.strip_prefix("deps-").is_some_and(|digest| {
                digest.len() >= 32 && digest.bytes().all(|b| b.is_ascii_hexdigit())
            });
        !cache_example && secret_token(token)
    }
}

pub(crate) fn ranges(text: &str) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut position = 0;
    while let Some(relative) = text[position..].find("-----BEGIN ") {
        let start = position + relative;
        let rest = &text[start..];
        let Some(header_end) = rest
            .find("-----\n")
            .map(|p| p + 6)
            .or_else(|| rest.find("-----\r\n").map(|p| p + 7))
        else {
            if rest.contains("PRIVATE KEY-----") {
                result.push(start..text.len());
            }
            break;
        };
        if rest[..header_end].contains("PRIVATE KEY-----") {
            let end = rest[header_end..]
                .find("-----END ")
                .and_then(|end| {
                    rest[header_end + end + 9..]
                        .find("-----")
                        .map(|tail| header_end + end + 9 + tail + 5)
                })
                .unwrap_or(rest.len());
            result.push(start..start + end);
            position = start + end;
        } else {
            position = start + header_end;
        }
        if position >= text.len() {
            break;
        }
    }
    // AWS secret access keys use the base64 alphabet, including '+' and '/'.
    let mut base64_start = None;
    for (index, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        if c.is_ascii_alphanumeric() || matches!(c, '+' | '/') {
            if base64_start.is_none() {
                base64_start = Some(index);
            }
        } else if let Some(begin) = base64_start.take()
            && index - begin == 40
        {
            let context = context_before(text, begin);
            if has_keyword(&context, "aws")
                && (has_keyword(&context, "secret") || has_keyword(&context, "key"))
                && !result.iter().any(|r| r.start < index && begin < r.end)
            {
                result.push(begin..index);
            }
        }
    }
    let mut start = None;
    for (index, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') {
            if start.is_none() {
                start = Some(index);
            }
        } else if let Some(begin) = start.take()
            && { contextual_token(&text[begin..index], &context_before(text, begin)) }
            && !result.iter().any(|r| r.start < index && begin < r.end)
        {
            result.push(begin..index);
        }
    }
    result.sort_by_key(|range| range.start);
    result
}

pub(crate) fn redact_detected(secret: &str) -> String {
    let mut output = String::from("****");
    if secret.chars().count() > 4 {
        let suffix_start = secret
            .char_indices()
            .rev()
            .nth(3)
            .map_or(secret.len(), |(i, _)| i);
        output.push_str(&secret[suffix_start..]);
    }
    output
}

/// Replaces detected credentials with a fixed marker.
#[must_use]
pub fn redact(text: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    for range in ranges(text) {
        output.push_str(&text[cursor..range.start]);
        output.push_str(&redact_detected(&text[range.clone()]));
        cursor = range.end;
    }
    output.push_str(&text[cursor..]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_detection_avoids_compound_prose_and_keeps_secrets() {
        for path in [
            "sk-learn-examples-for-agents/AGENTS.md",
            "dir/sk-aaaaaaaaaaaaaaaa/file",
            "dir/sk-proj-aaaaaaaaaaa/file",
            "dir/sk-ant-aaaaaaaaaaaa/file",
            "sk-hynix-memory-notes-2026/AGENTS.md",
            "AKIA-architecture-notes-folder-2026/AGENTS.md",
            "dir/AKIA1234567890ABCDE/file",
            "dir/AKIA1234567890ABCDEFG/file",
            "dir/AKIA1234567890abcdeF/file",
            "dir/ASIA1234567890ABCDE/file",
            "dir/ASIA1234567890ABCDEFG/file",
            "dir/ASIA1234567890abcdeF/file",
            "release-2026-11-05-team-review-v2b-final",
            "my-project-2026-11-05-release-candidate",
            "target-aarch64-unknown-linux-gnu-abcdef1234567890",
        ] {
            assert!(!path_has_secret(path), "{path}");
        }
        for path in [
            "dir/ghp_abcdefghijklmnopqrstuvwxyz1234567890/file",
            "dir/AKIA1234567890ABCDEF/file",
            "dir/AKIAAAAAAAAAAAAAAAAA/file",
            "dir/AKIA1234567890123456/file",
            "dir/ASIAAAAAAAAAAAAAAAAA/file",
            "dir/ASIA1234567890123456/file",
            "dir/sk-abcdefghijklmnopqrstuvwxyz1234567890/file",
            "dir/sk-aaaaaaaaaaaaaaaaa/file",
            "dir/sk-proj-aaaaaaaaaaaaa/file",
            "dir/sk-ant-aaaaaaaaaaaaaa/file",
            "dir/sk-proj-abcdefghijklmnopqrstuvwxyz1234567890/file",
            "dir/sk-ant-abcdefghijklmnopqrstuvwxyz1234567890/file",
            "dir/ASIA1234567890ABCDEF/file",
            "dir/xoxb-123456789012-abcdefghijklmnopqrstuvwxyz/file",
            "dir/Z8gP2mQ7vL4xN9aR6sT3wY5bC1dF0hKj/file",
        ] {
            assert!(path_has_secret(path), "{path}");
        }
    }

    #[test]
    fn unicode_context_starts_at_next_character_boundary() {
        let input = format!("é{}", "a".repeat(127));
        assert_eq!(context_before(&input, input.len()), "a".repeat(127));
    }

    #[test]
    fn contextual_aws_base64_requires_exactly_forty_characters() {
        let short = format!("aws_secret_access_key = {}", "a/".repeat(19) + "a");
        let exact = format!("aws_secret_access_key = {}", "a/".repeat(20));
        assert_eq!(ranges(&short), [] as [std::ops::Range<usize>; 0]);
        let found = ranges(&exact);
        assert_eq!(found.len(), 1);
        assert_eq!(&exact[found[0].clone()], "a/".repeat(20));
    }

    #[test]
    fn aws_context_requires_aws_and_key_or_secret() {
        let token = "a/".repeat(20);
        for context in ["aws key", "aws secret"] {
            let input = format!("{context}: {token}");
            let found = ranges(&input);
            assert_eq!(found.len(), 1, "{context}");
            assert_eq!(&input[found[0].clone()], token);
        }
        for context in ["aws region", "api key", "client secret"] {
            let input = format!("{context}: {token}");
            assert_eq!(
                ranges(&input),
                [] as [std::ops::Range<usize>; 0],
                "{context}"
            );
        }
    }

    #[test]
    fn aws_candidate_inside_pem_is_not_duplicated() {
        let token = "a/".repeat(20);
        let pem =
            format!("-----BEGIN PRIVATE KEY-----\naws key: {token}\n-----END PRIVATE KEY-----");
        assert_eq!(ranges(&pem), vec![0..pem.len()]);
    }

    #[test]
    fn aws_candidates_adjacent_to_pem_are_detected_separately() {
        let token = "a/".repeat(20);
        let pem = "-----BEGIN PRIVATE KEY-----\nbody\n-----END PRIVATE KEY-----";
        let before = format!("aws key: {token}{pem}");
        let found = ranges(&before);
        assert_eq!(found.len(), 2);
        assert_eq!(&before[found[0].clone()], token);
        assert_eq!(&before[found[1].clone()], pem);

        let after =
            format!("-----BEGIN PRIVATE KEY-----\nbody\naws key: -----END PRIVATE KEY-----{token}");
        let found = ranges(&after);
        assert_eq!(found.len(), 2);
        assert_eq!(
            &after[found[0].clone()],
            &after[..after.len() - token.len()]
        );
        assert_eq!(&after[found[1].clone()], token);
    }

    #[test]
    fn generic_tokens_adjacent_to_aws_candidates_stay_separate() {
        let aws = format!("/{}", "a".repeat(39));
        let before = format!("aws key: ghp_abcdefgh_{aws}");
        let found = ranges(&before);
        assert_eq!(found.len(), 2);
        assert_eq!(&before[found[0].clone()], "ghp_abcdefgh_");
        assert_eq!(&before[found[1].clone()], aws);

        let aws = format!("{}/", "a".repeat(39));
        let generic = "_Z8gP2mQ7vL4xN9aR6sT3wY5bC1dF0hKj";
        let after = format!("aws key: {aws}{generic}");
        let found = ranges(&after);
        assert_eq!(found.len(), 2);
        assert_eq!(&after[found[0].clone()], aws);
        assert_eq!(&after[found[1].clone()], generic);
    }

    #[test]
    fn aws_candidate_after_pem_is_detected_separately() {
        let token = "a/".repeat(20);
        let pem = "-----BEGIN PRIVATE KEY-----\nbody\n-----END PRIVATE KEY-----";
        let input = format!("{pem}\naws key: {token}");
        let found = ranges(&input);
        assert_eq!(found.len(), 2);
        assert_eq!(&input[found[0].clone()], pem);
        assert_eq!(&input[found[1].clone()], token);
    }

    #[test]
    fn detected_short_values_keep_only_allowed_suffix() {
        assert_eq!(redact_detected("abcd"), "****");
        assert_eq!(redact_detected("abcde"), "****bcde");
    }

    #[test]
    fn detected_contextual_values_are_masked_without_context() {
        for value in [
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "0123456789abcdef0123456789abcdef",
        ] {
            let masked = redact_detected(value);
            assert!(!masked.contains(value));
            assert_eq!(masked, format!("****{}", &value[value.len() - 4..]));
        }
    }

    #[test]
    fn known_families_are_redacted() {
        for secret in [
            "AKIA1234567890ABCDEF",
            "ghp_abcdefghijklmnopqrstuvwxyz1234567890",
            "sk-abcdefghijklmnopqrstuvwxyz1234567890",
            "sk-ant-abcdefghijklmnopqrstuvwxyz1234567890",
            "xoxb-123456789012-abcdefghijklmnopqrstuvwxyz",
            "-----BEGIN PRIVATE KEY-----",
        ] {
            let output = redact(&format!("prefix {secret} suffix"));
            assert!(!output.contains(secret), "secret leaked: {secret}");
            assert!(output.contains("****"));
        }
    }

    #[test]
    fn mask_keeps_only_the_last_four_characters() {
        for secret in [
            "AKIA1234567890ABCDEF",
            "ghp_abcdefghijklmnopqrstuvwxyz1234567890",
            "sk-ant-abcdefghijklmnopqrstuvwxyz1234567890",
            "xoxb-123456789012-abcdefghijklmnopqrstuvwxyz",
            "Z8gP2mQ7vL4xN9aR6sT3wY5bC1dF0hKj",
            "-----BEGIN PRIVATE KEY-----\nabc123XYZ\n-----END PRIVATE KEY-----",
        ] {
            let suffix: String = secret
                .chars()
                .rev()
                .take(4)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            assert_eq!(redact(secret), format!("****{suffix}"));
        }
    }
    #[test]
    fn entropy_and_nonsecret_inputs() {
        let secret = "Z8gP2mQ7vL4xN9aR6sT3wY5bC1dF0hKj";
        assert!(!redact(secret).contains(secret));
        assert_eq!(
            redact("ordinary explanatory words"),
            "ordinary explanatory words"
        );
    }
}
