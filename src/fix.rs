use anyhow::{Context, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(crate) fn apply(
    paths: &[(PathBuf, PathBuf)],
    write: bool,
    duplicate_enabled: bool,
    max_file_bytes: usize,
) -> Result<Vec<(PathBuf, usize)>> {
    let mut changed = Vec::new();
    for (path, display) in paths {
        let metadata = fs::symlink_metadata(path).context("cannot inspect input file")?;
        if !metadata.file_type().is_file() {
            continue;
        }
        let original = kanso::discovery::read_regular_bytes(path, max_file_bytes)
            .context("cannot read input file")?;
        let input = kanso::discovery::decode_regular_bytes(&original)
            .context("cannot decode input file")?;
        let output = clean(&input, duplicate_enabled);
        if input != output {
            let first_changed_line = input
                .split_inclusive('\n')
                .zip(output.split_inclusive('\n'))
                .position(|(before, after)| before != after)
                .unwrap_or_else(|| input.lines().count().min(output.lines().count()))
                + 1;
            changed.push((display.clone(), first_changed_line));
            if write {
                let parent = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                let mut temp = tempfile::NamedTempFile::new_in(parent)
                    .context("cannot create temporary file")?;
                temp.as_file()
                    .set_permissions(metadata.permissions())
                    .context("cannot preserve permissions")?;
                let bytes =
                    if original.starts_with(&[0xff, 0xfe]) || original.starts_with(&[0xfe, 0xff]) {
                        let little_endian = original[0] == 0xff;
                        let mut bytes = original[..2].to_vec();
                        for unit in output.encode_utf16() {
                            bytes.extend(if little_endian {
                                unit.to_le_bytes()
                            } else {
                                unit.to_be_bytes()
                            });
                        }
                        bytes
                    } else {
                        output.into_bytes()
                    };
                temp.write_all(&bytes)
                    .context("cannot write temporary file")?;
                temp.as_file()
                    .sync_all()
                    .context("cannot sync temporary file")?;
                if kanso::discovery::read_regular_bytes(path, max_file_bytes)? != original {
                    anyhow::bail!("input changed during fix");
                }
                temp.persist(path).context("cannot replace input file")?;
            }
        }
    }
    Ok(changed)
}

fn clean(input: &str, duplicate_enabled: bool) -> String {
    // Raw HTML can contain text whose whitespace and list markers are significant.
    if pulldown_cmark::Parser::new(input).any(|event| {
        matches!(
            event,
            pulldown_cmark::Event::Html(_) | pulldown_cmark::Event::InlineHtml(_)
        )
    }) {
        return input.to_owned();
    }
    let lines: Vec<&str> = input.split_inclusive('\n').collect();
    let mut result = String::with_capacity(input.len());
    let mut fence: Option<(char, usize)> = None;
    let mut frontmatter = lines.first().is_some_and(|line| {
        line.trim_start_matches('\u{feff}')
            .trim_end_matches(['\r', '\n'])
            == "---"
    });
    let mut previous: Option<String> = None;
    for (index, line) in lines.iter().enumerate() {
        let (body, ending) = match line.strip_suffix("\r\n") {
            Some(s) => (s, "\r\n"),
            None => match line.strip_suffix('\n') {
                Some(s) => (s, "\n"),
                None => (*line, ""),
            },
        };
        if frontmatter {
            result.push_str(line);
            if index > 0 && (body == "---" || body == "...") {
                frontmatter = false;
            }
            continue;
        }
        let trimmed = body.trim_start_matches(' ');
        let indent = body.len() - trimmed.len();
        let marker = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
        let run = marker.map_or(0, |m| trimmed.chars().take_while(|c| *c == m).count());
        if indent <= 3 && run >= 3 {
            match fence {
                None => fence = marker.map(|m| (m, run)),
                Some((m, n))
                    if marker == Some(m) && run >= n && trimmed[run..].trim().is_empty() =>
                {
                    fence = None;
                }
                _ => {}
            }
            result.push_str(line);
            previous = None;
            continue;
        }
        if fence.is_some() || indent >= 4 || body.starts_with('\t') {
            result.push_str(line);
            previous = None;
            continue;
        }
        let is_simple =
            (body.starts_with("- ") || body.starts_with("* ") || body.starts_with("+ "))
                && !body[2..].trim().is_empty();
        let body = if is_simple && body.ends_with(' ') && !body.ends_with("  ") {
            &body[..body.len() - 1]
        } else {
            body
        };
        let next_content = lines.iter().skip(index + 1).find(|s| !s.trim().is_empty());
        let indented_continuation =
            next_content.is_some_and(|s| s.starts_with(' ') || s.starts_with('\t'));
        let lazy_continuation = lines.get(index + 1).is_some_and(|s| {
            !s.trim().is_empty()
                && !s.starts_with("- ")
                && !s.starts_with("* ")
                && !s.starts_with("+ ")
                && !s.starts_with('#')
        });
        if duplicate_enabled
            && is_simple
            && !indented_continuation
            && !lazy_continuation
            && previous.as_deref() == Some(body)
        {
            continue;
        }
        previous = if is_simple {
            Some(body.to_owned())
        } else {
            None
        };
        result.push_str(body);
        result.push_str(ending);
    }
    result
}
