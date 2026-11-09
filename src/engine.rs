//! Orchestrates discovery and analysis.
use crate::{
    Config, Diagnostic, Document, FileReport, KansoError, Report, Severity, discovery, rules, score,
};
use std::path::{Path, PathBuf};

/// Select the nearest project marker above the common input directory.
///
/// # Errors
/// Returns an error if an input cannot be canonicalized or the current directory is unavailable.
pub fn project_root(paths: &[PathBuf]) -> Result<PathBuf, KansoError> {
    let inputs = if paths.is_empty() {
        vec![std::env::current_dir()?]
    } else {
        paths.to_vec()
    };
    let mut common: Option<PathBuf> = None;
    for input in inputs {
        let path = match std::fs::symlink_metadata(&input) {
            Ok(meta) if meta.file_type().is_symlink() => input
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .canonicalize()?,
            _ => match input.canonicalize() {
                Ok(path) => path,
                Err(error) if std::fs::symlink_metadata(&input).is_ok() => input
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .unwrap_or(Path::new("."))
                    .canonicalize()
                    .map_err(|_| error)?,
                Err(error) => return Err(error.into()),
            },
        };
        let directory = if path.is_dir() {
            path.as_path()
        } else {
            path.parent().unwrap_or(&path)
        };
        if let Some(current) = &mut common {
            while !directory.starts_with(&*current) {
                if !current.pop() {
                    break;
                }
            }
        } else {
            common = Some(directory.to_path_buf());
        }
    }
    let common = common.ok_or_else(|| KansoError::Config("no input root".into()))?;
    Ok(common
        .ancestors()
        .find(|dir| dir.join(".git").exists() || dir.join("kanso.toml").is_file())
        .unwrap_or(&common)
        .to_path_buf())
}
fn io_diagnostic(path: PathBuf, message: &str) -> Diagnostic {
    Diagnostic {
        rule_id: "KS000".into(),
        severity: Severity::Error,
        message: message.into(),
        path,
        line: 1,
        column: 1,
    }
}
fn root_instruction(path: &Path, root: &Path) -> bool {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let Ok(rel) = canonical.strip_prefix(root) else {
        return false;
    };
    let parts: Vec<_> = rel.components().collect();
    if parts.len() == 1 {
        return matches!(
            rel.to_str(),
            Some("AGENTS.md" | "CLAUDE.md" | "GEMINI.md" | ".cursorrules" | ".windsurfrules")
        );
    }
    if rel == Path::new(".github/copilot-instructions.md") {
        return true;
    }
    rel.parent() == Some(Path::new(".cursor/rules")) && rel.extension().is_some_and(|x| x == "mdc")
}
fn alias_diagnostic(path: PathBuf) -> Diagnostic {
    Diagnostic {
        rule_id: "alias".into(),
        severity: Severity::Info,
        message: "symbolic link alias".into(),
        path,
        line: 1,
        column: 1,
    }
}
fn project_report_paths(
    reports: &mut [FileReport],
    diagnostics: &mut [Diagnostic],
    spellings: &[(PathBuf, PathBuf)],
) {
    for file in reports {
        if let Some((_, spelling)) = spellings
            .iter()
            .find(|(canonical, _)| canonical == &file.path)
        {
            file.path.clone_from(spelling);
        }
    }
    for diagnostic in diagnostics {
        if let Some((_, spelling)) = spellings
            .iter()
            .find(|(canonical, _)| canonical == &diagnostic.path)
        {
            diagnostic.path.clone_from(spelling);
        }
    }
}
fn visible_path_secret_diagnostics(
    spellings: &[(PathBuf, PathBuf)],
    diagnostics: &[Diagnostic],
) -> Vec<Diagnostic> {
    let visible_files: Vec<_> = spellings.iter().map(|(_, path)| path.clone()).collect();
    let mut result = path_secret_diagnostics(&visible_files, diagnostics);
    for diagnostic in &mut result {
        if let Some((canonical, _)) = spellings
            .iter()
            .find(|(_, spelling)| spelling == &diagnostic.path)
        {
            diagnostic.path.clone_from(canonical);
        }
    }
    result
}
fn path_secret_diagnostics(files: &[PathBuf], diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
    let paths: std::collections::BTreeSet<_> = files
        .iter()
        .chain(diagnostics.iter().map(|d| &d.path))
        .collect();
    paths
        .into_iter()
        .filter(|path| {
            path.components().any(|part| {
                let text = part.as_os_str().to_string_lossy();
                rules::path_has_secret(&text)
            })
        })
        .map(|path| Diagnostic {
            rule_id: "KS007".into(),
            severity: Severity::Warn,
            message: "secret-like token in path".into(),
            path: path.clone(),
            line: 1,
            column: 1,
        })
        .collect()
}
/// Check discovered files. Project root is the nearest ancestor of the common
/// input directory containing `.git` or `kanso.toml`, or the common directory.
/// Only supported root instructions participate in cross-file checks.
///
/// # Errors
/// Returns an error for invalid config, filesystem failures or rule failures.
pub fn check(paths: &[PathBuf], config: &Config) -> Result<Report, KansoError> {
    config.validate()?;
    let root = project_root(paths)?;
    let (files, failures, aliases, spellings) =
        discovery::discover_with_aliases(paths, config, &root)?;
    let mut documents = Vec::with_capacity(files.len());
    let mut diagnostics: Vec<Diagnostic> = failures
        .into_iter()
        .map(|path| {
            let message = if std::fs::symlink_metadata(&path)
                .is_ok_and(|m| m.file_type().is_symlink())
            {
                "symbolic link skipped"
            } else if std::fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file() && !m.is_dir()) {
                "not a regular file"
            } else {
                "cannot inspect or traverse path"
            };
            io_diagnostic(path, message)
        })
        .collect();
    diagnostics.extend(aliases.into_iter().map(alias_diagnostic));
    if !config.disabled_rules.iter().any(|rule| rule == "KS007") {
        diagnostics.extend(visible_path_secret_diagnostics(&spellings, &diagnostics));
    }
    for path in files {
        match discovery::read_regular(&path, config.max_file_bytes) {
            Ok(source) => documents.push(Document { path, source }),
            Err(error) => {
                let message = match error.kind() {
                    std::io::ErrorKind::PermissionDenied => "permission denied",
                    std::io::ErrorKind::InvalidData
                        if error.to_string() == "file exceeds size limit" =>
                    {
                        "file exceeds size limit"
                    }
                    std::io::ErrorKind::InvalidData if error.to_string() == "binary file" => {
                        "binary file"
                    }
                    std::io::ErrorKind::InvalidData
                        if error.to_string() == "UTF-16/unsupported encoding" =>
                    {
                        "UTF-16/unsupported encoding"
                    }
                    std::io::ErrorKind::InvalidData => "invalid UTF-8 file",
                    std::io::ErrorKind::InvalidInput => "not a regular file",
                    _ => "cannot read file safely",
                };
                diagnostics.push(io_diagnostic(path, message));
            }
        }
    }
    let mut reports = Vec::new();
    let mut total_tokens = 0_usize;
    for doc in &documents {
        let tokens = rules::token_count(&doc.source)?;
        total_tokens = total_tokens.saturating_add(tokens);
        let local = rules::analyze(doc, config, &root)?;
        reports.push(FileReport {
            path: doc.path.clone(),
            tokens,
            score: score::score(&local),
        });
        diagnostics.extend(local);
    }
    let roots: Vec<Document> = documents
        .iter()
        .filter(|d| root_instruction(&d.path, &root))
        .cloned()
        .collect();
    let combined = rules::analyze_combined(&roots, config, &root)?;
    for file in &mut reports {
        let mut local: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.path == file.path)
            .cloned()
            .collect();
        local.extend(combined.iter().filter(|d| d.path == file.path).cloned());
        file.score = score::score(&local);
    }
    diagnostics.extend(combined);
    project_report_paths(&mut reports, &mut diagnostics, &spellings);
    diagnostics.sort_by(|a, b| {
        (&a.path, a.line, a.column, &a.rule_id, &a.message)
            .cmp(&(&b.path, b.line, b.column, &b.rule_id, &b.message))
    });
    let score = score::score(&diagnostics);
    Ok(Report {
        schema: "kanso/v1".into(),
        files: reports,
        diagnostics,
        score,
        total_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_input_uses_nearest_project_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kanso.toml"), "").unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        assert_eq!(
            project_root(&[dir.path().to_path_buf()]).unwrap(),
            dir.path().canonicalize().unwrap()
        );
    }
    #[test]
    fn root_context_excludes_nested_and_skill() {
        let root = Path::new("/project");
        assert!(root_instruction(&root.join("AGENTS.md"), root));
        assert!(root_instruction(&root.join(".cursor/rules/test.mdc"), root));
        assert!(!root_instruction(&root.join("nested/AGENTS.md"), root));
        assert!(!root_instruction(&root.join("SKILL.md"), root));
    }
}
