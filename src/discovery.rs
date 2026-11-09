//! Deterministic discovery of supported instruction files.
use crate::{Config, KansoError};
use ignore::{WalkBuilder, gitignore::GitignoreBuilder};
use std::path::{Path, PathBuf};

fn supported(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(
        name,
        "AGENTS.md" | "CLAUDE.md" | "GEMINI.md" | "SKILL.md" | ".cursorrules" | ".windsurfrules"
    ) || (name == "copilot-instructions.md"
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == ".github"))
        || (path.extension().is_some_and(|ext| ext == "mdc")
            && path.parent().is_some_and(|p| p.ends_with(".cursor/rules")))
}
/// Discover recognized instruction files, respecting gitignore even outside git repositories.
/// Explicit files are accepted regardless of their names.
///
/// # Errors
/// Returns an error when a path, ignore pattern, or traversal is invalid.
pub fn discover(paths: &[PathBuf], config: &Config) -> Result<Vec<PathBuf>, KansoError> {
    discover_with_failures(paths, config).map(|(files, _)| files)
}

/// Discover canonical identities and their first caller-visible paths.
///
/// # Errors
/// Returns an error when a path, ignore pattern, or traversal is invalid.
pub fn discover_with_paths(
    paths: &[PathBuf],
    config: &Config,
) -> Result<Vec<(PathBuf, PathBuf)>, KansoError> {
    let root = crate::engine::project_root(paths)
        .or_else(|_| std::env::current_dir().map_err(KansoError::from))?;
    let (_, _, _, mut spellings) = discover_with_aliases(paths, config, &root)?;
    spellings.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(spellings)
}

/// Read and decode a regular UTF-8 or BOM-marked UTF-16 file, bounded by `max_bytes`.
///
/// # Errors
/// Returns an I/O error for non-regular files, unsupported encodings, oversized files or failed reads.
pub fn read_regular(path: &Path, max_bytes: usize) -> std::io::Result<String> {
    decode_regular_bytes(&read_regular_bytes(path, max_bytes)?)
}

/// Read a regular file without following its final symlink, bounded by `max_bytes`.
///
/// # Errors
/// Returns an error for non-regular files, oversized files or failed reads.
pub fn read_regular_bytes(path: &Path, max_bytes: usize) -> std::io::Result<Vec<u8>> {
    use std::io::{self, Read};
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    let limit = u64::try_from(max_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file exceeds size limit",
        ));
    }
    Ok(bytes)
}

/// Decode UTF-8 or BOM-marked UTF-16 bytes.
///
/// # Errors
/// Returns an error for invalid or unsupported encodings or binary content.
pub fn decode_regular_bytes(bytes: &[u8]) -> std::io::Result<String> {
    use std::io;
    let utf16_error = || io::Error::new(io::ErrorKind::InvalidData, "UTF-16/unsupported encoding");
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little_endian = bytes[0] == 0xff;
        let data = &bytes[2..];
        if !data.len().is_multiple_of(2) {
            return Err(utf16_error());
        }
        let units: Vec<u16> = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| {
                if little_endian {
                    u16::from_le_bytes(pair)
                } else {
                    u16::from_be_bytes(pair)
                }
            })
            .collect();
        return String::from_utf16(&units).map_err(|_| utf16_error());
    }
    if bytes.contains(&0) {
        let pairs = bytes.as_chunks::<2>().0;
        let count = pairs.len();
        if count >= 2
            && (pairs.iter().filter(|p| p[0] == 0).count() * 2 >= count
                || pairs.iter().filter(|p| p[1] == 0).count() * 2 >= count)
        {
            return Err(utf16_error());
        }
        return Err(io::Error::new(io::ErrorKind::InvalidData, "binary file"));
    }
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn error_path(error: &ignore::Error) -> Option<&Path> {
    match error {
        ignore::Error::WithPath { path, .. } => Some(path),
        ignore::Error::WithDepth { err, .. } | ignore::Error::WithLineNumber { err, .. } => {
            error_path(err)
        }
        ignore::Error::Loop { child, .. } => Some(child),
        _ => None,
    }
}

pub(crate) fn discover_with_failures(
    paths: &[PathBuf],
    config: &Config,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), KansoError> {
    let root = match crate::engine::project_root(paths) {
        Ok(root) => root,
        Err(_) => std::env::current_dir()?,
    };
    discover_with_aliases(paths, config, &root).map(|(files, failures, _, spellings)| {
        (
            files
                .into_iter()
                .map(|p| {
                    spellings
                        .iter()
                        .find(|(identity, _)| identity == &p)
                        .map_or(p.clone(), |(_, spelling)| spelling.clone())
                })
                .collect(),
            failures,
        )
    })
}

type DiscoveryResult = (
    Vec<PathBuf>,
    Vec<PathBuf>,
    Vec<PathBuf>,
    Vec<(PathBuf, PathBuf)>,
);

pub(crate) fn discover_with_aliases(
    paths: &[PathBuf],
    config: &Config,
    root: &Path,
) -> Result<DiscoveryResult, KansoError> {
    let inputs = if paths.is_empty() {
        vec![std::env::current_dir()?]
    } else {
        paths.to_vec()
    };
    let mut found = Vec::new();
    let mut failures = Vec::new();
    let mut aliases = Vec::new();
    let mut spellings = Vec::new();
    let mut failure_identities = std::collections::HashSet::new();
    let mut alias_identities = std::collections::HashSet::new();
    for input in inputs {
        let metadata = match std::fs::symlink_metadata(&input) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(e.into()),
            Err(_) => {
                if failure_identities.insert(entry_identity(&input)) {
                    failures.push(input);
                }
                continue;
            }
        };
        let mut input_found = Vec::new();
        let mut input_failures = Vec::new();
        let mut input_aliases = Vec::new();
        let (canonical, spelling) = if metadata.is_dir() {
            let path = input.canonicalize()?;
            walk_directory(
                &path,
                config,
                root,
                &mut input_found,
                &mut input_failures,
                &mut input_aliases,
            )?;
            (path, input.clone())
        } else {
            if metadata.is_file() {
                input_found.push((input.canonicalize()?, input.clone()));
            } else if metadata.file_type().is_symlink() {
                record_alias(
                    &input,
                    root,
                    &mut input_found,
                    &mut input_failures,
                    &mut input_aliases,
                );
            } else {
                input_failures.push(input.clone());
            }
            (
                input.canonicalize().unwrap_or_else(|_| input.clone()),
                input.clone(),
            )
        };
        for (file, discovered) in input_found {
            if !spellings.iter().any(|(identity, _)| identity == &file) {
                let display = if metadata.is_dir() {
                    project_path(discovered, &[(canonical.clone(), spelling.clone())])
                } else {
                    input.clone()
                };
                spellings.push((file.clone(), display));
                found.push(file);
            }
        }
        for failure in input_failures {
            if failure_identities.insert(entry_identity(&failure)) {
                failures.push(if metadata.is_dir() {
                    project_path(failure, &[(canonical.clone(), spelling.clone())])
                } else {
                    failure
                });
            }
        }
        for alias in input_aliases {
            if alias_identities.insert(entry_identity(&alias)) {
                aliases.push(if metadata.is_dir() {
                    project_path(alias, &[(canonical.clone(), spelling.clone())])
                } else {
                    alias
                });
            }
        }
    }
    found.sort();
    failures.sort();
    aliases.sort();
    Ok((found, failures, aliases, spellings))
}

fn walk_directory(
    path: &Path,
    config: &Config,
    root: &Path,
    input_found: &mut Vec<(PathBuf, PathBuf)>,
    input_failures: &mut Vec<PathBuf>,
    input_aliases: &mut Vec<PathBuf>,
) -> Result<(), KansoError> {
    let mut overrides = GitignoreBuilder::new(path);
    for pattern in &config.ignore {
        overrides
            .add_line(None, pattern)
            .map_err(|e| KansoError::Config(e.to_string()))?;
    }
    let overrides = overrides
        .build()
        .map_err(|e| KansoError::Config(e.to_string()))?;
    let mut walker = WalkBuilder::new(path);
    walker
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| {
            if !entry.file_type().is_some_and(|t| t.is_dir()) {
                return true;
            }
            !matches!(
                entry.file_name().to_str(),
                Some(".git" | "target" | "node_modules" | "vendor" | ".venv")
            )
        });
    // `Walk` yields raw directory order, which varies with the filesystem; sort so the first
    // spelling recorded for a canonical identity does not depend on it.
    let mut entries: Vec<ignore::DirEntry> = Vec::new();
    let mut traversal_failures = Vec::new();
    for entry in walker.build() {
        match entry {
            Ok(entry) => {
                let entry_path = entry.path();
                if supported(entry_path)
                    && !overrides
                        .matched_path_or_any_parents(entry_path, false)
                        .is_ignore()
                {
                    entries.push(entry);
                }
            }
            Err(error) => {
                traversal_failures.push(error_path(&error).unwrap_or(path).to_path_buf());
            }
        }
    }
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    traversal_failures.sort();
    for entry in entries {
        let entry_path = entry.path();
        if entry.file_type().is_some_and(|t| t.is_file()) {
            match entry_path.canonicalize() {
                Ok(canonical) => input_found.push((canonical, entry_path.to_path_buf())),
                Err(_) => input_failures.push(entry_path.to_path_buf()),
            }
        } else if entry.file_type().is_some_and(|t| t.is_symlink()) {
            record_alias(entry_path, root, input_found, input_failures, input_aliases);
        } else {
            input_failures.push(entry_path.to_path_buf());
        }
    }
    input_failures.append(&mut traversal_failures);
    Ok(())
}

fn entry_identity(path: &Path) -> PathBuf {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    match (parent.canonicalize(), path.file_name()) {
        (Ok(parent), Some(name)) => parent.join(name),
        _ => path.to_path_buf(),
    }
}

fn project_path(path: PathBuf, spellings: &[(PathBuf, PathBuf)]) -> PathBuf {
    spellings
        .iter()
        .find_map(|(canonical, spelling)| {
            path.strip_prefix(canonical).ok().map(|suffix| {
                if suffix.as_os_str().is_empty() {
                    spelling.clone()
                } else {
                    spelling.join(suffix)
                }
            })
        })
        .unwrap_or(path)
}

fn record_alias(
    path: &Path,
    root: &Path,
    found: &mut Vec<(PathBuf, PathBuf)>,
    failures: &mut Vec<PathBuf>,
    aliases: &mut Vec<PathBuf>,
) {
    match path.canonicalize() {
        Ok(target)
            if target.starts_with(root)
                && std::fs::metadata(&target).is_ok_and(|m| m.is_file()) =>
        {
            found.push((target, path.to_path_buf()));
            aliases.push(path.to_path_buf());
        }
        _ => failures.push(path.to_path_buf()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traversal_errors_preserve_the_affected_path() {
        let path = PathBuf::from("nested/locked");
        let wrapped = ignore::Error::WithDepth {
            depth: 2,
            err: Box::new(ignore::Error::WithLineNumber {
                line: 3,
                err: Box::new(ignore::Error::WithPath {
                    path: path.clone(),
                    err: Box::new(ignore::Error::Io(std::io::Error::from(
                        std::io::ErrorKind::PermissionDenied,
                    ))),
                }),
            }),
        };
        assert_eq!(error_path(&wrapped), Some(path.as_path()));
        let loop_error = ignore::Error::Loop {
            ancestor: PathBuf::from("parent"),
            child: path.clone(),
        };
        assert_eq!(error_path(&loop_error), Some(path.as_path()));
        let no_path = ignore::Error::Io(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert_eq!(error_path(&no_path), None);
    }
    #[test]
    fn only_expected_special_paths_are_supported() {
        assert!(supported(Path::new("/repo/.cursor/rules/example.mdc")));
        assert!(supported(Path::new(
            "/repo/.github/copilot-instructions.md"
        )));
        assert!(!supported(Path::new("/repo/other/example.mdc")));
        assert!(!supported(Path::new("/repo/other/copilot-instructions.md")));
    }
}
