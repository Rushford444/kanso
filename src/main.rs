mod fix;
mod output;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use kanso::{Config, discovery, engine, rules};
use serde_json::json;
use std::{fs::OpenOptions, io::Write, path::PathBuf};

#[derive(Parser)]
#[command(
    name = "kanso",
    version,
    about = "Lint AI instruction files",
    after_help = "Exit codes: 0 success; 1 findings; 2 usage error; 3 runtime error.\nExamples: kanso check .; kanso score --badge; kanso fix --write"
)]
struct Cli {
    /// Emit schema-versioned JSON.
    #[arg(long, global = true)]
    json: bool,
    /// Suppress human success output.
    #[arg(long, global = true)]
    quiet: bool,
    /// Do not prompt for confirmation.
    #[arg(long, global = true)]
    yes: bool,
    /// Configuration path; init creates this file.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check instruction files. Findings exit with code 1.
    #[command(
        after_help = "Examples: kanso check .\nExit codes: 0 success; 1 findings; 2 usage error; 3 runtime error."
    )]
    Check {
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
        #[arg(long, value_enum)]
        format: Option<Format>,
    },
    /// Compute scores and optionally a static Markdown badge.
    #[command(
        after_help = "Examples: kanso score --badge\nExit codes: 0 success; 2 usage error; 3 runtime error."
    )]
    Score {
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
        #[arg(long)]
        badge: bool,
    },
    /// Explain a rule, for example KS001.
    #[command(
        after_help = "Examples: kanso explain KS001\nExit codes: 0 success; 2 usage error; 3 runtime error."
    )]
    Explain { rule: String },
    /// Create a default kanso.toml without replacing an existing file.
    #[command(
        after_help = "Examples: kanso init\nExit codes: 0 success; 2 usage error; 3 runtime error."
    )]
    Init,
    /// Preview conservative whitespace and duplicate fixes; use --write to apply.
    #[command(
        after_help = "Examples: kanso fix --dry-run\nExit codes: 0 success; 2 usage error; 3 runtime error."
    )]
    Fix {
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
        #[arg(long, conflicts_with = "dry_run")]
        write: bool,
        /// Preview changes without writing (the default).
        #[arg(long)]
        dry_run: bool,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Sarif,
}

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let json_requested = args.iter().any(|arg| arg == "--json");
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => {
            use clap::error::ErrorKind;
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                if error.print().is_err() {
                    std::process::exit(3);
                }
                return;
            }
            error_output(json_requested, "usage", "invalid command or arguments");
            std::process::exit(2);
        }
    };
    let machine = cli.json;
    let paths = match &cli.command {
        Command::Check { paths, .. }
        | Command::Score { paths, .. }
        | Command::Fix { paths, .. } => paths.as_slice(),
        _ => &[],
    };
    if paths.iter().any(|path| {
        std::fs::symlink_metadata(path)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    }) {
        error_output(machine, "usage", "input path does not exist");
        std::process::exit(2);
    }
    match run(cli) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            let config_error = error.downcast_ref::<kanso::KansoError>();
            let message = if let Some(kanso::KansoError::Config(message)) = config_error {
                // Config strings can include untrusted TOML; only fixed validation errors are displayed.
                match message.as_str() {
                    "warning threshold must not exceed error threshold"
                    | "similarity threshold must be between 0 and 1"
                    | "max_file_bytes must be greater than zero" => message.as_str(),
                    value if value.starts_with("unknown disabled rule:") => "unknown disabled rule",
                    _ => "invalid configuration",
                }
            } else {
                "cannot complete operation; check input paths and permissions"
            };
            let is_config = matches!(config_error, Some(kanso::KansoError::Config(_)));
            error_output(
                machine,
                if is_config { "usage" } else { "runtime" },
                message,
            );
            std::process::exit(if is_config { 2 } else { 3 });
        }
    }
}

fn error_output(machine: bool, code: &str, message: &str) {
    if machine {
        output::json(&mut json!({"schema":"kanso/v1","error":{"code":code,"message":message}}));
    } else {
        eprintln!("{code}: {message}");
    }
}

fn load_config(path: Option<PathBuf>, paths: &[PathBuf]) -> Result<Config> {
    if let Some(path) = path {
        return Ok(Config::load(&path)?);
    }
    let path = engine::project_root(paths)?.join("kanso.toml");
    if path.exists() {
        Ok(Config::load(&path)?)
    } else {
        Ok(Config::default())
    }
}

fn init(machine: bool, quiet: bool, config: Option<PathBuf>) -> Result<i32> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(config.unwrap_or_else(|| PathBuf::from("kanso.toml")));
    let created = match file {
        Ok(mut file) => {
            file.write_all(b"# kanso configuration; uncomment to override the defaults.\n# Warning and error thresholds for a single file, in estimated tokens.\n# token_warn = 2000\n# token_error = 6000\n# Aggregate root-context limits.\n# total_token_warn = 10000\n# total_token_error = 20000\n# similarity_threshold = 0.85\n# max_file_bytes = 2097152\n# disabled_rules = []\n# ignore = []\n")?;
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error.into()),
    };
    if machine {
        output::json(&mut json!({"schema":"kanso/v1","created":created}));
    } else if !quiet {
        println!(
            "{}",
            if created {
                "Created kanso.toml"
            } else {
                "kanso.toml already exists"
            }
        );
    }
    Ok(0)
}

fn run_fix(
    paths: &[PathBuf],
    write: bool,
    machine: bool,
    quiet: bool,
    config: Option<PathBuf>,
) -> Result<i32> {
    if write {
        for path in paths {
            for ancestor in path.ancestors() {
                if std::fs::symlink_metadata(ancestor).is_ok_and(|m| m.file_type().is_symlink()) {
                    anyhow::bail!("symlink input is not permitted for writes");
                }
            }
        }
    }
    let config = load_config(config, paths)?;
    let duplicate_enabled = !config.disabled_rules.iter().any(|rule| rule == "KS005");
    let paths = discovery::discover_with_paths(paths, &config)?;
    let changed = fix::apply(&paths, write, duplicate_enabled, config.max_file_bytes)
        .context("fix failed")?;
    if machine {
        let files: Vec<_> = changed.iter().map(|(path, _)| path).collect();
        output::json(
            &mut json!({"schema":"kanso/v1","changed_files":changed.len(),"files":files,"written":write}),
        );
    } else if !quiet {
        println!(
            "{} file(s) {}",
            changed.len(),
            if write {
                "updated"
            } else {
                "would change (dry run)"
            }
        );
    }
    if !machine && !quiet {
        for (path, line) in &changed {
            if write {
                println!("{}", path.display());
            } else {
                println!("{}:{line}: changes start here", path.display());
            }
        }
    }
    Ok(0)
}

fn run(cli: Cli) -> Result<i32> {
    let Cli {
        json: machine,
        quiet,
        yes: _,
        config,
        command,
    } = cli;
    match command {
        Command::Check { paths, format } => {
            let report = engine::check(&paths, &load_config(config, &paths)?)?;
            if format.is_some() {
                output::json(&mut output::sarif(&report));
            } else if machine {
                output::json(&mut serde_json::to_value(&report)?);
            } else if !quiet {
                output::text(&report);
            }
            Ok(i32::from(
                report.diagnostics.iter().any(|d| d.rule_id != "alias"),
            ))
        }
        Command::Score { paths, badge } => {
            let report = engine::check(&paths, &load_config(config, &paths)?)?;
            let color = if report.score >= 80 {
                "brightgreen"
            } else if report.score >= 50 {
                "yellow"
            } else {
                "red"
            };
            let url = format!(
                "https://img.shields.io/badge/kanso-{}%2F100-{color}",
                report.score
            );
            let markdown = format!("![kanso score]({url})");
            if machine {
                output::json(
                    &mut json!({"schema":"kanso/v1","score":report.score,"files":report.files,"total_tokens":report.total_tokens,"badge":if badge { Some(markdown) } else { None }}),
                );
            } else if !quiet {
                if badge {
                    println!("{markdown}");
                } else {
                    for file in &report.files {
                        println!("{}: {} / 100", file.path.display(), file.score);
                    }
                    println!("Global: {} / 100", report.score);
                }
            }
            Ok(0)
        }
        Command::Explain { rule } => {
            let Some(explanation) = rules::explain(&rule) else {
                error_output(machine, "usage", "unknown rule");
                return Ok(2);
            };
            if machine {
                output::json(
                    &mut json!({"schema":"kanso/v1","rule":rule,"explanation":explanation}),
                );
            } else if !quiet {
                println!("{}", rules::redact(explanation));
            }
            Ok(0)
        }
        Command::Init => init(machine, quiet, config),
        Command::Fix {
            paths,
            write,
            dry_run: _,
        } => run_fix(&paths, write, machine, quiet, config),
    }
}
