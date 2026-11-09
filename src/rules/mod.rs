//! Deterministic checks for instruction documents.
mod secrets;
pub(crate) use secrets::path_has_secret;
pub use secrets::redact;

use crate::{
    KansoError,
    config::Config,
    model::{Diagnostic, Document, Severity},
    parser,
};
use std::{collections::HashSet, path::Path, sync::OnceLock};

/// Count tokens using the bundled o200k vocabulary.
///
/// # Errors
/// Returns an error if the tokenizer cannot initialize.
pub fn token_count(text: &str) -> Result<usize, KansoError> {
    static BPE: OnceLock<Result<tiktoken_rs::CoreBPE, String>> = OnceLock::new();
    let bpe = BPE.get_or_init(|| tiktoken_rs::o200k_base().map_err(|e| e.to_string()));
    match bpe {
        Ok(bpe) => Ok(bpe.encode_ordinary(text).len()),
        Err(e) => Err(KansoError::Tokenizer(e.clone())),
    }
}

/// Stable rule identifiers with their short name and one-line description, in
/// SARIF `rules` order.
///
/// Each entry is `(id, name, short description)`. The long description is not
/// repeated here: [`explain`] stays its single source.
pub const RULE_METADATA: [(&str, &str, &str); 11] = [
    (
        "alias",
        "Alias",
        "A symbolic link points to an already analyzed instruction file.",
    ),
    (
        "KS000",
        "UnreadableInput",
        "An input could not be read safely and was skipped.",
    ),
    (
        "KS001",
        "SkillMetadata",
        "SKILL.md frontmatter name or description is missing or invalid.",
    ),
    (
        "KS002",
        "TokenBudget",
        "The file exceeds its configured o200k token budget.",
    ),
    (
        "KS003",
        "ContextBudget",
        "The combined root instruction context exceeds its token budget.",
    ),
    (
        "KS004",
        "MissingLink",
        "A relative Markdown link target or anchor was not found.",
    ),
    (
        "KS005",
        "DuplicateItem",
        "A normalized list item repeats an earlier instruction.",
    ),
    (
        "KS006",
        "SimilarItem",
        "A list item resembles an earlier item under SimHash similarity.",
    ),
    (
        "KS007",
        "PossibleSecret",
        "A possible plaintext credential was detected and redacted.",
    ),
    (
        "KS008",
        "HeadingStructure",
        "Title, heading levels, or section content is missing.",
    ),
    (
        "KS009",
        "UndeclaredCommand",
        "A referenced project command is not declared or standard.",
    ),
];

/// Explain a stable rule identifier or informational diagnostic.
#[must_use]
pub fn explain(id: &str) -> Option<&'static str> {
    match id {
        "alias" => Some(
            "A symbolic link points to an instruction file already analyzed. The alias is informational and does not affect the score or exit status.",
        ),
        "KS000" => Some(
            "A file or directory could not be read safely. Causes include permissions, symlinks, invalid UTF-8, binary content, or the configured size limit. The affected item is skipped and analysis continues.",
        ),
        "KS001" => Some(
            "SKILL.md requires YAML name and description. Name uses lowercase ASCII letters, digits and hyphens and is at most 64 characters; description is nonempty and at most 1024 characters.",
        ),
        "KS002" => Some(
            "The file exceeds the configured o200k token budget. Adjust token_warn and token_error in kanso.toml to change the limits.",
        ),
        "KS003" => {
            Some("The combined root instruction context exceeds its configured o200k token budget.")
        }
        "KS004" => Some("A relative Markdown link refers to a missing file or heading anchor."),
        "KS005" => Some(
            "A normalized list item repeats an earlier instruction within its file or co-loaded root files.",
        ),
        "KS006" => Some(
            "A list item resembles an earlier item according to SimHash and the configured threshold.",
        ),
        "KS007" => Some("A possible plaintext credential was detected; values are redacted."),
        "KS008" => {
            Some("The document has no title, skips heading levels, or has an empty section.")
        }
        "KS009" => Some(
            "A referenced project command is not declared in package.json, Makefile, or justfile, or is not a standard cargo command.",
        ),
        _ => None,
    }
}
fn normalize_list(text: &str) -> String {
    let text = text.trim();
    let text = text
        .strip_prefix(['-', '*', '+'])
        .or_else(|| {
            text.find('.')
                .filter(|&i| text[..i].bytes().all(|b| b.is_ascii_digit()))
                .map(|i| &text[i + 1..])
        })
        .unwrap_or(text);
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn simhash(text: &str) -> u64 {
    let mut bits = [0_i32; 64];
    for word in text.split_whitespace() {
        let hash = word.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
        for (i, bit) in bits.iter_mut().enumerate() {
            *bit += if hash & (1_u64 << i) != 0 { 1 } else { -1 };
        }
    }
    bits.iter().enumerate().fold(
        0,
        |acc, (i, bit)| if *bit > 0 { acc | (1_u64 << i) } else { acc },
    )
}
fn similarity(a: &str, b: &str) -> f64 {
    1.0 - f64::from((simhash(a) ^ simhash(b)).count_ones()) / 64.0
}
fn diagnostic(
    doc: &Document,
    id: &str,
    severity: Severity,
    message: &str,
    line: usize,
    column: usize,
) -> Diagnostic {
    Diagnostic {
        rule_id: id.into(),
        severity,
        message: message.into(),
        path: doc.path.clone(),
        line,
        column,
    }
}
fn enabled(config: &Config, id: &str) -> bool {
    !config.disabled_rules.iter().any(|x| x == id)
}
fn position(text: &str, offset: usize) -> (usize, usize) {
    let prefix = text.get(..offset).unwrap_or("");
    (
        prefix.bytes().filter(|&b| b == b'\n').count() + 1,
        prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
    )
}
fn budget(
    doc: &Document,
    id: &str,
    tokens: usize,
    warn: usize,
    error: usize,
) -> Option<Diagnostic> {
    let severity = if tokens > error {
        Severity::Error
    } else if tokens > warn {
        Severity::Warn
    } else {
        return None;
    };
    Some(diagnostic(
        doc,
        id,
        severity,
        &format!("token budget exceeded ({tokens} tokens)"),
        1,
        1,
    ))
}
fn skill(doc: &Document, parsed: &parser::ParsedDocument, out: &mut Vec<Diagnostic>) {
    #[derive(serde::Deserialize)]
    struct SkillFields {
        name: Option<String>,
        description: Option<String>,
    }
    if doc.path.file_name().and_then(|s| s.to_str()) != Some("SKILL.md") {
        return;
    }
    let front = parsed
        .frontmatter
        .as_ref()
        .and_then(|s| doc.source.get(s.start..s.end));
    let fields = front
        .map(|f| f.trim_start_matches('\u{feff}'))
        .and_then(|f| f.split_once('\n').map(|(_, body)| body))
        .and_then(|body| {
            body.trim_end_matches(['\r', '\n'])
                .rsplit_once('\n')
                .map(|(value, _)| value)
        })
        .and_then(|f| serde_saphyr::from_str::<serde_json::Value>(f).ok())
        .and_then(|value| serde_json::from_value::<SkillFields>(value).ok());
    let name = fields.as_ref().and_then(|v| v.name.as_deref());
    let description = fields.as_ref().and_then(|v| v.description.as_deref());
    if !name.is_some_and(|n| {
        !n.is_empty()
            && n.len() <= 64
            && n.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    }) || !description.is_some_and(|d| !d.trim().is_empty() && d.chars().count() <= 1024)
    {
        out.push(diagnostic(
            doc,
            "KS001",
            Severity::Error,
            "invalid or missing skill name or description",
            1,
            1,
        ));
    }
}
fn slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c)
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}
fn decode_percent(text: &str) -> String {
    let mut bytes = Vec::with_capacity(text.len());
    let mut iter = text.as_bytes().iter().copied().peekable();
    while let Some(byte) = iter.next() {
        if byte == b'%' {
            let mut probe = iter.clone();
            if let (Some(a), Some(b)) = (probe.next(), probe.next())
                && let (Some(a), Some(b)) = (char::from(a).to_digit(16), char::from(b).to_digit(16))
            {
                bytes.push(u8::try_from(a * 16 + b).unwrap_or_default());
                iter = probe;
                continue;
            }
        }
        bytes.push(byte);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
fn has_anchor(parsed: &parser::ParsedDocument, anchor: &str) -> bool {
    let mut counts = std::collections::HashMap::new();
    parsed.headings.iter().any(|heading| {
        let base = slug(&heading.text);
        let count = counts.entry(base.clone()).or_insert(0_usize);
        let current = if *count == 0 {
            base
        } else {
            format!("{base}-{count}")
        };
        *count += 1;
        current == anchor
    })
}
fn links(
    doc: &Document,
    parsed: &parser::ParsedDocument,
    root: &Path,
    max_bytes: usize,
    out: &mut Vec<Diagnostic>,
) {
    for link in &parsed.links {
        let target = link.destination.as_str();
        if target.is_empty()
            || target.starts_with('/')
            || target.starts_with("//")
            || target.split_once(':').is_some_and(|(scheme, _)| {
                scheme
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphabetic)
                    && scheme
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
            })
        {
            continue;
        }
        let (file, anchor) = target
            .split_once('#')
            .map_or((target, None), |(f, a)| (f, Some(a)));
        let file = file.split_once('?').map_or(file, |(path, _)| path);
        let path = if file.is_empty() {
            doc.path.clone()
        } else {
            doc.path
                .parent()
                .unwrap_or(Path::new("."))
                .join(decode_percent(file))
        };
        let exists = if file.is_empty() { true } else { path.exists() };
        let anchor_exists = if let Some(anchor) = anchor.filter(|a| !a.is_empty()) {
            let anchor = decode_percent(anchor);
            if file.is_empty() {
                has_anchor(parsed, &anchor)
            } else {
                path.canonicalize()
                    .ok()
                    .zip(root.canonicalize().ok())
                    .filter(|(target, root)| target.starts_with(root))
                    .and_then(|(target, _)| crate::discovery::read_regular(&target, max_bytes).ok())
                    .is_some_and(|content| has_anchor(&parser::parse(&content), &anchor))
            }
        } else {
            true
        };
        if !exists || !anchor_exists {
            out.push(diagnostic(
                doc,
                "KS004",
                Severity::Warn,
                "relative link target or anchor not found",
                link.span.line,
                link.span.column,
            ));
        }
    }
}
fn structure(doc: &Document, parsed: &parser::ParsedDocument, out: &mut Vec<Diagnostic>) {
    if !parsed.headings.iter().any(|h| h.level == 1) {
        out.push(diagnostic(
            doc,
            "KS008",
            Severity::Warn,
            "document has no level-one title",
            1,
            1,
        ));
    }
    let mut prev = 0;
    for (i, h) in parsed.headings.iter().enumerate() {
        if prev > 0 && h.level > prev + 1 {
            out.push(diagnostic(
                doc,
                "KS008",
                Severity::Warn,
                "heading level skipped",
                h.span.line,
                h.span.column,
            ));
        }
        let end = parsed
            .headings
            .iter()
            .skip(i + 1)
            .find(|next| next.level <= h.level)
            .map_or(doc.source.len(), |next| next.span.start);
        if doc
            .source
            .get(h.span.end..end)
            .is_some_and(|section| section.trim().is_empty())
        {
            out.push(diagnostic(
                doc,
                "KS008",
                Severity::Info,
                "empty section",
                h.span.line,
                h.span.column,
            ));
        }
        prev = h.level;
    }
}
fn duplicate_items(
    doc: &Document,
    parsed: &parser::ParsedDocument,
    config: &Config,
    seen: &mut Vec<String>,
    seen_set: &mut HashSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    for item in &parsed.list_items {
        let Some(raw) = doc.source.get(item.start..item.end) else {
            continue;
        };
        let normalized = normalize_list(raw);
        if normalized.is_empty() {
            continue;
        }
        if seen_set.contains(&normalized) {
            if enabled(config, "KS005") {
                out.push(diagnostic(
                    doc,
                    "KS005",
                    Severity::Warn,
                    "duplicate list instruction",
                    item.line,
                    item.column,
                ));
            }
        } else if enabled(config, "KS006")
            && normalized.split_whitespace().count() >= 3
            && seen.iter().any(|previous| {
                previous.split_whitespace().count() >= 3
                    && similarity(previous, &normalized) >= config.similarity_threshold
            })
        {
            out.push(diagnostic(
                doc,
                "KS006",
                Severity::Info,
                "similar list instruction",
                item.line,
                item.column,
            ));
        }
        seen_set.insert(normalized.clone());
        seen.push(normalized);
    }
}
fn command_targets(root: &Path, name: &str, max_bytes: usize) -> HashSet<String> {
    let mut targets = HashSet::new();
    if let Ok(text) = crate::discovery::read_regular(&root.join(name), max_bytes) {
        for line in text.lines() {
            if line.starts_with(char::is_whitespace) {
                continue;
            }
            let Some((target, _)) = line.split_once(':') else {
                continue;
            };
            if target.contains('#') {
                continue;
            }
            if name == "Makefile" && !target.contains('=') {
                targets.extend(target.split_whitespace().map(str::to_owned));
            } else if name == "justfile"
                && let Some(recipe) = target
                    .split_whitespace()
                    .next()
                    .filter(|s| !s.contains('='))
            {
                targets.insert(recipe.to_owned());
            }
        }
    }
    targets
}
fn invalid_command(
    cargo_parts: &[&str],
    scripts: &HashSet<String>,
    make: &HashSet<String>,
    just: &HashSet<String>,
) -> bool {
    match cargo_parts {
        ["npm" | "pnpm" | "yarn", "run", name, ..]
        | ["npm", name @ ("test" | "start" | "stop" | "restart"), ..] => !scripts.contains(*name),
        ["pnpm" | "yarn", name, ..] if !name.starts_with('-') => {
            ![
                "install",
                "i",
                "add",
                "remove",
                "rm",
                "update",
                "up",
                "upgrade",
                "exec",
                "dlx",
                "create",
                "init",
                "help",
                "version",
                "config",
                "list",
                "ls",
                "why",
                "audit",
                "outdated",
                "import",
                "prune",
                "store",
                "link",
                "unlink",
                "pack",
                "publish",
                "rebuild",
                "patch",
                "set",
                "get",
                "cache",
                "workspaces",
                "workspace",
                "node",
                "bin",
                "run",
            ]
            .contains(name)
                && !scripts.contains(*name)
        }
        ["make", name, ..] if !name.starts_with('-') => !make.contains(*name),
        ["just", name, ..] if !name.starts_with('-') => !just.contains(*name),
        ["cargo", name, ..] if !name.starts_with('-') => ![
            "build",
            "check",
            "test",
            "run",
            "bench",
            "doc",
            "fmt",
            "clippy",
            "nextest",
            "deny",
            "insta",
            "mutants",
            "clean",
            "metadata",
            "fetch",
            "tree",
            "update",
            "install",
            "uninstall",
            "login",
            "logout",
            "owner",
            "yank",
            "publish",
            "package",
            "new",
            "init",
            "add",
            "remove",
            "search",
            "version",
            "help",
            "generate-lockfile",
            "locate-project",
            "read-manifest",
            "verify-project",
            "vendor",
            "fix",
            "rustc",
            "rustdoc",
        ]
        .contains(name),
        _ => false,
    }
}
fn workspace_scripts(root: &Path, max_bytes: usize, scripts: &mut HashSet<String>) {
    let mut patterns = Vec::new();
    if let Ok(text) = crate::discovery::read_regular(&root.join("package.json"), max_bytes)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
    {
        let workspaces = value.get("workspaces");
        let packages = workspaces.and_then(|value| {
            value
                .as_array()
                .or_else(|| value.get("packages").and_then(serde_json::Value::as_array))
        });
        if let Some(packages) = packages {
            patterns.extend(
                packages
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned),
            );
        }
    }
    if let Ok(text) = crate::discovery::read_regular(&root.join("pnpm-workspace.yaml"), max_bytes)
        && let Ok(value) = serde_saphyr::from_str::<serde_json::Value>(&text)
        && let Some(packages) = value.get("packages").and_then(serde_json::Value::as_array)
    {
        patterns.extend(
            packages
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned),
        );
    }
    if patterns.is_empty() {
        return;
    }
    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    for pattern in patterns {
        let (negation, pattern) = pattern
            .strip_prefix('!')
            .map_or(("", pattern.as_str()), |p| ("!", p));
        let pattern = pattern.trim_start_matches("./").trim_start_matches('/');
        if pattern.is_empty() || pattern.split('/').any(|p| p == "..") {
            continue;
        }
        let _ = builder.add_line(None, &format!("{negation}/{pattern}"));
    }
    let Ok(patterns) = builder.build() else {
        return;
    };
    let mut walker = ignore::WalkBuilder::new(root);
    walker
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| {
            !entry.file_type().is_some_and(|t| t.is_dir())
                || !matches!(
                    entry.file_name().to_str(),
                    Some(".git" | "target" | "node_modules" | "vendor" | ".venv")
                )
        });
    for entry in walker.build().flatten() {
        if entry.file_type().is_some_and(|t| t.is_dir())
            && patterns.matched(entry.path(), true).is_ignore()
            && let Ok(text) =
                crate::discovery::read_regular(&entry.path().join("package.json"), max_bytes)
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
            && let Some(map) = value.get("scripts").and_then(serde_json::Value::as_object)
        {
            scripts.extend(map.keys().cloned());
        }
    }
}
fn commands(
    doc: &Document,
    parsed: &parser::ParsedDocument,
    root: &Path,
    max_bytes: usize,
    out: &mut Vec<Diagnostic>,
) {
    let mut scripts = HashSet::new();
    let document_dir = doc.path.parent().unwrap_or(root);
    let root_canonical = root.canonicalize().ok();
    let doc_canonical = document_dir.canonicalize().ok();
    if let (Some(root), Some(dir)) = (root_canonical, doc_canonical)
        && dir.starts_with(&root)
    {
        for ancestor in dir.ancestors().take_while(|p| p.starts_with(&root)) {
            let manifest = ancestor.join("package.json");
            if manifest.exists() {
                if let Ok(text) = crate::discovery::read_regular(&manifest, max_bytes)
                    && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
                    && let Some(map) = value.get("scripts").and_then(serde_json::Value::as_object)
                {
                    scripts.extend(map.keys().cloned());
                }
                break;
            }
        }
    }
    if document_dir.canonicalize().ok() == root.canonicalize().ok() && root.canonicalize().is_ok() {
        workspace_scripts(root, max_bytes, &mut scripts);
    }
    let make = command_targets(root, "Makefile", max_bytes);
    let just = command_targets(root, "justfile", max_bytes);
    for fence in &parsed.fences {
        if !matches!(
            fence.info.split_whitespace().next(),
            Some("sh" | "bash" | "shell")
        ) {
            continue;
        }
        let Some(text) = doc.source.get(fence.span.start..fence.span.end) else {
            continue;
        };
        for (line_no, line) in text.lines().enumerate().skip(1) {
            let line = line.trim().trim_start_matches("$ ");
            for command in line.split(&[';', '&'][..]).filter(|s| !s.trim().is_empty()) {
                let mut parts: Vec<_> = command.split_whitespace().collect();
                while parts.first().is_some_and(|part| {
                    part.split_once('=').is_some_and(|(name, _)| {
                        !name.is_empty()
                            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    })
                }) {
                    parts.remove(0);
                }
                if parts.first() == Some(&"sudo") {
                    parts.remove(0);
                }
                let cargo_parts = if parts.first() == Some(&"cargo")
                    && parts.get(1).is_some_and(|part| part.starts_with('+'))
                {
                    [&parts[..1], &parts[2..]].concat()
                } else {
                    parts.clone()
                };
                let invalid = invalid_command(&cargo_parts, &scripts, &make, &just);
                if invalid {
                    out.push(diagnostic(
                        doc,
                        "KS009",
                        Severity::Info,
                        "project command not found",
                        fence.span.line + line_no,
                        1,
                    ));
                }
            }
        }
    }
}
/// Analyze one document without network or command execution.
///
/// # Errors
/// Returns an error when the tokenizer cannot initialize.
pub fn analyze(
    doc: &Document,
    config: &Config,
    root: &Path,
) -> Result<Vec<Diagnostic>, KansoError> {
    let parsed = parser::parse(&doc.source);
    let mut out = Vec::new();
    if enabled(config, "KS001") {
        skill(doc, &parsed, &mut out);
    }
    if enabled(config, "KS002")
        && let Some(d) = budget(
            doc,
            "KS002",
            token_count(&doc.source)?,
            config.token_warn,
            config.token_error,
        )
    {
        out.push(d);
    }
    if enabled(config, "KS004") {
        links(doc, &parsed, root, config.max_file_bytes, &mut out);
    }
    duplicate_items(
        doc,
        &parsed,
        config,
        &mut Vec::new(),
        &mut HashSet::new(),
        &mut out,
    );
    if enabled(config, "KS007") {
        for range in secrets::ranges(&doc.source) {
            let (line, column) = position(&doc.source, range.start);
            out.push(diagnostic(
                doc,
                "KS007",
                Severity::Error,
                &format!(
                    "possible secret: {}",
                    secrets::redact_detected(&doc.source[range])
                ),
                line,
                column,
            ));
        }
    }
    if enabled(config, "KS008") {
        structure(doc, &parsed, &mut out);
    }
    if enabled(config, "KS009") {
        commands(doc, &parsed, root, config.max_file_bytes, &mut out);
    }
    Ok(out)
}
/// Analyze additional combined-root checks for already co-loaded documents.
///
/// # Errors
/// Returns an error when the tokenizer cannot initialize.
pub fn analyze_combined(
    docs: &[Document],
    config: &Config,
    _root: &Path,
) -> Result<Vec<Diagnostic>, KansoError> {
    let mut out = Vec::new();
    if let Some(first) = docs.first() {
        if enabled(config, "KS003") {
            let mut total = 0_usize;
            for d in docs {
                total = total.saturating_add(token_count(&d.source)?);
            }
            if let Some(d) = budget(
                first,
                "KS003",
                total,
                config.total_token_warn,
                config.total_token_error,
            ) {
                out.push(d);
            }
        }
        let mut prior = Vec::new();
        let mut prior_set = HashSet::new();
        for doc in docs {
            let parsed = parser::parse(&doc.source);
            let mut local = Vec::new();
            for item in &parsed.list_items {
                let Some(raw) = doc.source.get(item.start..item.end) else {
                    continue;
                };
                let n = normalize_list(raw);
                if n.is_empty() {
                    continue;
                }
                if prior_set.contains(&n) {
                    if enabled(config, "KS005") {
                        out.push(diagnostic(
                            doc,
                            "KS005",
                            Severity::Warn,
                            "duplicate list instruction in root context",
                            item.line,
                            item.column,
                        ));
                    }
                } else if enabled(config, "KS006")
                    && n.split_whitespace().count() >= 3
                    && prior.iter().any(|p: &String| {
                        p.split_whitespace().count() >= 3
                            && similarity(p, &n) >= config.similarity_threshold
                    })
                {
                    out.push(diagnostic(
                        doc,
                        "KS006",
                        Severity::Info,
                        "similar list instruction in root context",
                        item.line,
                        item.column,
                    ));
                }
                local.push(n);
            }
            prior_set.extend(local.iter().cloned());
            prior.extend(local);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn list_normalization_ignores_marker_case_and_spacing() {
        assert_eq!(
            normalize_list("- Use   Cargo test"),
            normalize_list("1. use cargo test")
        );
        assert_ne!(
            normalize_list("- use cargo test"),
            normalize_list("- use cargo build")
        );
    }
    fn doc(path: &Path, source: &str) -> Document {
        Document {
            path: path.into(),
            source: source.into(),
        }
    }
    fn has(ds: &[Diagnostic], id: &str) -> bool {
        ds.iter().any(|d| d.rule_id == id)
    }
    #[test]
    fn checks_skills_budgets_and_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let valid = doc(
            &root.join("SKILL.md"),
            "---\nname: safe-name\ndescription: Useful instructions\n---\n# Title\n\nSome text.\n",
        );
        let invalid = doc(
            &root.join("SKILL.md"),
            "---\nname: Bad_Name\ndescription: ''\n---\n### Skip\n",
        );
        let config = Config::default();
        let good = analyze(&valid, &config, root).unwrap();
        let bad = analyze(&invalid, &config, root).unwrap();
        assert!(!has(&good, "KS001"));
        assert!(has(&bad, "KS001"));
        assert!(!has(&good, "KS008"));
        assert!(has(&bad, "KS008"));
        let small = Config {
            token_warn: 0,
            token_error: 1,
            total_token_warn: 0,
            total_token_error: 1,
            ..config
        };
        assert!(has(&analyze(&valid, &small, root).unwrap(), "KS002"));
        assert!(has(
            &analyze_combined(&[valid], &small, root).unwrap(),
            "KS003"
        ));
        assert!(!has(&analyze_combined(&[], &small, root).unwrap(), "KS003"));
    }
    #[test]
    fn checks_relative_links_and_root_duplicates() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("target.md"), "# Existing heading\n").unwrap();
        let one = doc(
            &root.join("AGENTS.md"),
            "# Title\n\n[ok](target.md#existing-heading)\n[bad](target.md#missing)\n\n- always run all project tests\n",
        );
        let two = doc(
            &root.join("CLAUDE.md"),
            "# Title\n\n- ALWAYS run all project tests\n",
        );
        let config = Config::default();
        let ds = analyze(&one, &config, root).unwrap();
        assert_eq!(ds.iter().filter(|d| d.rule_id == "KS004").count(), 1);
        assert!(!has(&analyze(&two, &config, root).unwrap(), "KS005"));
        assert!(has(
            &analyze_combined(&[one, two], &config, root).unwrap(),
            "KS005"
        ));
    }
    #[test]
    fn anchors_handle_encoded_paths_and_repeated_headings() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("space name.md"),
            "# Hello world\n\n# Hello world\n",
        )
        .unwrap();
        let document = doc(
            &root.join("AGENTS.md"),
            "# Title\n\n[link](space%20name.md#hello-world-1)\n",
        );
        assert!(!has(
            &analyze(&document, &Config::default(), root).unwrap(),
            "KS004"
        ));
    }
    #[test]
    fn explanations_cover_every_rule_and_reject_unknown_ids() {
        for id in [
            "alias", "KS000", "KS001", "KS002", "KS003", "KS004", "KS005", "KS006", "KS007",
            "KS008", "KS009",
        ] {
            let explanation = explain(id).unwrap();
            assert!(explanation.len() > 30, "{id} explanation is not useful");
        }
        for (id, name, short) in RULE_METADATA {
            assert!(
                name.bytes().all(|b| b.is_ascii_alphanumeric()),
                "{id} name {name} is not alphanumeric"
            );
            assert!(!short.is_empty(), "{id} has no short description");
            assert!(short.len() <= 120, "{id} short description is too long");
            assert!(short.is_ascii(), "{id} short description is not ASCII");
            let long = explain(id).unwrap();
            assert!(
                long.len() > short.len(),
                "{id} full description is not longer"
            );
        }
        for id in ["", "KS010", "ks001", "KS0010"] {
            assert_eq!(explain(id), None);
        }
    }
    #[test]
    fn disabled_rules_and_token_count() {
        let tmp = tempfile::tempdir().unwrap();
        let document = doc(
            &tmp.path().join("AGENTS.md"),
            "# Title\n\n- same rule\n- same rule\n",
        );
        let config = Config {
            disabled_rules: vec!["KS005".into(), "KS006".into()],
            ..Config::default()
        };
        let findings = analyze(&document, &config, tmp.path()).unwrap();
        assert!(!has(&findings, "KS005"));
        assert!(!has(&findings, "KS006"));
        assert_eq!(token_count("").unwrap(), 0);
        assert!(token_count("hello world").unwrap() > 0);
    }
    #[test]
    fn known_project_commands_and_toolchain_overrides_are_valid() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("Makefile"), "test build: deps\n\ttrue\n").unwrap();
        std::fs::write(
            root.join("justfile"),
            "test filter='':\n  echo {{filter}}\n",
        )
        .unwrap();
        let document = doc(
            &root.join("AGENTS.md"),
            "# Title\n\n```sh\ncargo login\ncargo +nightly test\nmake test\nmake build\njust test\ncargo not-a-command\nmake nonexistent\njust missing\n```\n",
        );
        let found = analyze(&document, &Config::default(), root).unwrap();
        let lines: Vec<usize> = found
            .iter()
            .filter(|d| d.rule_id == "KS009")
            .map(|d| d.line)
            .collect();
        assert_eq!(lines, vec![9, 10, 11]);
    }
    #[test]
    fn verified_semantic_regressions() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("exists.md"), "# heading\n").unwrap();
        let markdown = "# Parent\n\n## Child\nContent.\n\n[query](exists.md?raw=1#heading)\n[scheme](tel:12345)\n[missing](missing.md?x=1)\n\n```sh\nFOO=1 cargo nope\ncd sub && cargo nope\nnpm test\n```\n";
        let document = doc(&root.join("AGENTS.md"), markdown);
        let findings = analyze(&document, &Config::default(), root).unwrap();
        assert_eq!(findings.iter().filter(|d| d.rule_id == "KS004").count(), 1);
        assert!(
            !findings
                .iter()
                .any(|d| d.rule_id == "KS008" && d.line == 1 && d.message == "empty section")
        );
        assert_eq!(findings.iter().filter(|d| d.rule_id == "KS009").count(), 3);
        for value in ["123", "true"] {
            let source = format!("---\nname: {value}\ndescription: Fine\n---\n# Title\ncontent\n");
            assert!(has(
                &analyze(
                    &doc(&root.join("SKILL.md"), &source),
                    &Config::default(),
                    root
                )
                .unwrap(),
                "KS001"
            ));
        }
        let source = "\u{feff}---\nname: valid\ndescription: Fine\n...\n# Title\ncontent\n";
        assert!(!has(
            &analyze(
                &doc(&root.join("SKILL.md"), source),
                &Config::default(),
                root
            )
            .unwrap(),
            "KS001"
        ));
        assert_eq!(redact("sk-learn-more"), "sk-learn-more");
    }
    #[test]
    fn checks_commands_secrets_and_near_duplicates() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("package.json"), r#"{"scripts":{"test":"ok"}}"#).unwrap();
        let d = doc(
            &root.join("AGENTS.md"),
            "# Title\n\n- always run project test suite\n- always run project test suites\n\n```sh\nnpm run test\nnpm run absent\n```\n\nkey sk-abcdefghijklmnopqrstuvwxyz1234567890\n",
        );
        let c = Config {
            similarity_threshold: 0.5,
            ..Config::default()
        };
        let ds = analyze(&d, &c, root).unwrap();
        assert!(has(&ds, "KS006"));
        assert!(has(&ds, "KS007"));
        let secret = ds.iter().find(|d| d.rule_id == "KS007").unwrap();
        assert!(secret.message.contains("****7890"));
        assert_eq!(ds.iter().filter(|d| d.rule_id == "KS009").count(), 1);
        assert!(!format!("{ds:?}").contains("sk-abcdefghijklmnopqrstuvwxyz1234567890"));
    }
    #[test]
    fn rule_switch_positions_and_budget_boundaries() {
        let doc = doc(Path::new("AGENTS.md"), "");
        let mut config = Config::default();
        assert!(enabled(&config, "KS002"));
        config.disabled_rules.push("KS002".into());
        assert!(!enabled(&config, "KS002"));
        assert!(enabled(&config, "KS003"));
        assert_eq!(position("a\néclair!", 5), (2, 3));
        assert!(budget(&doc, "KS002", 10, 10, 20).is_none());
        assert_eq!(
            budget(&doc, "KS002", 11, 10, 20).unwrap().severity,
            Severity::Warn
        );
        assert_eq!(
            budget(&doc, "KS002", 20, 10, 20).unwrap().severity,
            Severity::Warn
        );
        assert_eq!(
            budget(&doc, "KS002", 21, 10, 20).unwrap().severity,
            Severity::Error
        );
    }
    #[test]
    fn skill_fields_are_independently_required_and_bounded() {
        let root = Path::new(".");
        for (name, description, valid) in [
            ("okay", "Fine", true),
            ("Bad", "Fine", false),
            ("okay", "", false),
            ("", "Fine", false),
            ("bad_name", "Fine", false),
        ] {
            let source = format!(
                "---\nname: '{name}'\ndescription: '{description}'\n---\n# Title\ncontent\n"
            );
            let document = doc(Path::new("SKILL.md"), &source);
            assert_eq!(
                has(
                    &analyze(&document, &Config::default(), root).unwrap(),
                    "KS001"
                ),
                !valid
            );
        }
        for source in [
            format!(
                "---\nname: {}\ndescription: fine\n---\n# Title\n",
                "a".repeat(65)
            ),
            format!(
                "---\nname: okay\ndescription: {}\n---\n# Title\n",
                "a".repeat(1025)
            ),
        ] {
            assert!(has(
                &analyze(
                    &doc(Path::new("SKILL.md"), &source),
                    &Config::default(),
                    root
                )
                .unwrap(),
                "KS001"
            ));
        }
    }
    #[test]
    fn anchor_slug_and_url_decoding() {
        assert_eq!(slug("One-two_three!"), "one-two_three");
        assert_eq!(decode_percent("a%2Fb%20c"), "a/b c");
        assert_eq!(decode_percent("a%QZ"), "a%QZ");
    }
    #[test]
    fn simhash_identical_and_different() {
        let text = "run the complete project test suite";
        let identical = similarity(text, text);
        assert!((identical - 1.0).abs() < f64::EPSILON);
        assert!(
            similarity(
                "run the complete project test suite",
                "never ship production secrets anywhere"
            ) < 0.95
        );
    }
}
