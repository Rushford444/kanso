use kanso::{Report, Severity, rules};
use serde_json::{Value, json};

pub(crate) fn sanitize(value: &mut Value) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(sanitize),
        Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(key.as_str(), "message" | "text" | "explanation") {
                    if let Value::String(text) = value {
                        *text = rules::redact(text);
                    } else {
                        sanitize(value);
                    }
                } else {
                    sanitize(value);
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn json(value: &mut Value) {
    sanitize(value);
    println!("{value}");
}

pub(crate) fn text(report: &Report) {
    for diagnostic in &report.diagnostics {
        let severity = match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warn => "warning",
            Severity::Info => "info",
        };
        println!(
            "{}:{}:{}: {} {}: {}",
            diagnostic.path.to_string_lossy(),
            diagnostic.line,
            diagnostic.column,
            severity,
            diagnostic.rule_id,
            rules::redact(&diagnostic.message)
                .replace('\n', "\\n")
                .replace('\r', "\\r")
        );
    }
    println!(
        "Score: {} / 100 ({} tokens)",
        report.score, report.total_tokens
    );
}

fn uri(path: &str) -> String {
    uri_for_platform(path, cfg!(windows))
}

fn uri_for_platform(path: &str, windows: bool) -> String {
    let path = if windows {
        path.replace('\\', "/")
    } else {
        path.to_owned()
    };
    let path = if windows {
        if let Some(rest) = path.strip_prefix("//?/UNC/") {
            format!("//{rest}")
        } else if let Some(rest) = path.strip_prefix("//?/") {
            rest.to_owned()
        } else {
            path
        }
    } else {
        path
    };
    let drive = windows
        && path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && path.as_bytes().get(1) == Some(&b':')
        && path.as_bytes().get(2) == Some(&b'/');
    let unc = windows && path.starts_with("//");
    let mut result = if drive {
        String::from("file:///")
    } else if unc {
        String::from("file:")
    } else if !windows && path.starts_with('/') {
        String::from("file://")
    } else {
        String::new()
    };
    for (i, b) in path.bytes().enumerate() {
        if b.is_ascii_alphanumeric()
            || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~')
            || (drive && i == 1)
        {
            result.push(char::from(b));
        } else {
            use std::fmt::Write;
            let _ = write!(&mut result, "%{b:02X}");
        }
    }
    result
}

const HELP_URI: &str = "https://github.com/Rushford444/kanso#what-is-checked";

fn described_rules() -> Vec<(&'static str, Value)> {
    rules::RULE_METADATA
        .into_iter()
        .filter_map(|(id, name, short)| {
            let description = rules::explain(id)?;
            Some((
                id,
                json!({
                    "id": id,
                    "name": name,
                    "shortDescription": {"text": short},
                    "fullDescription": {"text": description},
                    "helpUri": HELP_URI,
                }),
            ))
        })
        .collect()
}

pub(crate) fn sarif(report: &Report) -> Value {
    let described = described_rules();
    let results: Vec<_> = report.diagnostics.iter().filter(|d| !(d.rule_id == "alias" && d.severity == Severity::Info)).map(|d| {
        let level = match d.severity { Severity::Error => "error", Severity::Warn => "warning", Severity::Info => "note" };
        let mut result = json!({"ruleId":d.rule_id, "level":level, "message":{"text":rules::redact(&d.message)}, "locations":[{"physicalLocation":{"artifactLocation":{"uri":uri(&d.path.to_string_lossy())},"region":{"startLine":d.line.max(1),"startColumn":d.column.max(1)}}}]});
        if let Some(index) = described.iter().position(|(id, _)| *id == d.rule_id) {
            result["ruleIndex"] = json!(index);
        }
        result
    }).collect();
    json!({"$schema":"https://json.schemastore.org/sarif-2.1.0.json","version":"2.1.0","runs":[{"tool":{"driver":{"name":"kanso","version":env!("CARGO_PKG_VERSION"),"rules":described.iter().map(|(_, rule)| rule).collect::<Vec<_>>()}},"results":results}]})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_examples() {
        let windows = [
            (r"C:\proj\AGENTS.md", "file:///C:/proj/AGENTS.md"),
            (
                r"C:\PROGRA~1\é #%.md",
                "file:///C:/PROGRA~1/%C3%A9%20%23%25.md",
            ),
            (r"\\server\share\Agent.md", "file://server/share/Agent.md"),
            (r"\\?\C:\proj\AGENTS.md", "file:///C:/proj/AGENTS.md"),
            (
                r"\\?\UNC\server\share\Agent.md",
                "file://server/share/Agent.md",
            ),
            (r"sub\AGENTS.md", "sub/AGENTS.md"),
            (r"C:AGENTS.md", "C%3AAGENTS.md"),
        ];
        for (path, expected) in windows {
            assert_eq!(uri_for_platform(path, true), expected, "{path}");
        }
        for (path, expected) in [
            ("/tmp/a:b #%.md", "file:///tmp/a%3Ab%20%23%25.md"),
            ("//tmp/file", "file:////tmp/file"),
            ("dir/a:b", "dir/a%3Ab"),
            ("ab/file", "ab/file"),
            ("x:y", "x%3Ay"),
            ("./é\\name", "./%C3%A9%5Cname"),
        ] {
            assert_eq!(uri_for_platform(path, false), expected, "{path}");
        }
    }

    #[test]
    fn sanitize_masks_nested_messages_but_preserves_paths() {
        let secret = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
        let mut value = json!({
            "path": secret,
            "nested": [{"message": format!("credential: {secret}"), "uri": secret}],
            "message": {"text": format!("credential: {secret}")},
            "explanation": format!("credential: {secret}")
        });
        sanitize(&mut value);
        assert_eq!(value["path"], secret);
        assert_eq!(value["nested"][0]["uri"], secret);
        for message in [
            &value["nested"][0]["message"],
            &value["message"]["text"],
            &value["explanation"],
        ] {
            let text = message.as_str().unwrap();
            assert!(!text.contains(secret));
            assert!(text.contains("****"));
        }
    }
}
