use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
  name = "rusty",
  version,
  about = "Opinionated Rust formatting and linting tools"
)]
struct Cli {
  #[command(subcommand)]
  command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
  /// Run Rusty lints against a project path, directory, or file.
  Check {
    /// Paths to check. Defaults to the current directory.
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
  },
  /// Format Rust source files with Rusty formatter rules.
  Format {
    /// Report files that need formatting without writing changes.
    #[arg(long)]
    check: bool,

    /// Paths to format. Defaults to the current directory.
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
  },
}

fn main() -> ExitCode {
  let cli = Cli::parse();

  match cli.command.unwrap_or(Command::Check {
    paths: vec![PathBuf::from(".")],
  }) {
    Command::Check { paths } => run_check(paths),
    Command::Format { check, paths } => run_format(paths, check),
  }
}

fn run_check(paths: Vec<PathBuf>) -> ExitCode {
  let mut had_error = false;
  let mut diagnostics_count = 0;

  for path in paths {
    match check_path(&path) {
      Ok(count) => {
        diagnostics_count += count;
      }
      Err(error) => {
        eprintln!("{}: {error}", path.display());
        had_error = true;
      }
    }
  }

  if diagnostics_count > 0 {
    return ExitCode::from(1);
  }

  if had_error {
    return ExitCode::from(2);
  }

  ExitCode::SUCCESS
}

fn check_path(path: &Path) -> Result<usize, String> {
  if path.is_dir() {
    let mut count = 0;

    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
      let entry = entry.map_err(|error| error.to_string())?;
      let path = entry.path();

      if path.file_name().is_some_and(is_ignored_directory) {
        continue;
      }

      count += check_path(&path)?;
    }

    return Ok(count);
  }

  if path.extension().is_none_or(|extension| extension != "rs") {
    return Ok(0);
  }

  let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
  let diagnostics = rusty_core::check_file(path, &source);

  for diagnostic in &diagnostics {
    eprintln!(
      "{}:{}:{} {} {}",
      path.display(),
      diagnostic.line,
      diagnostic.column,
      diagnostic.rule_id,
      diagnostic.message
    );
  }

  Ok(diagnostics.len())
}

fn run_format(paths: Vec<PathBuf>, check: bool) -> ExitCode {
  let mut had_error = false;
  let mut formatted_count = 0;

  for path in paths {
    match format_path(&path, check) {
      Ok(count) => {
        formatted_count += count;
      }
      Err(error) => {
        eprintln!("{}: {error}", path.display());
        had_error = true;
      }
    }
  }

  if check && formatted_count > 0 {
    return ExitCode::from(1);
  }

  if had_error {
    return ExitCode::from(2);
  }

  ExitCode::SUCCESS
}

fn format_path(path: &Path, check: bool) -> Result<usize, String> {
  if path.is_dir() {
    let mut count = 0;

    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
      let entry = entry.map_err(|error| error.to_string())?;
      let path = entry.path();

      if path.file_name().is_some_and(is_ignored_directory) {
        continue;
      }

      count += format_path(&path, check)?;
    }

    return Ok(count);
  }

  if path.extension().is_none_or(|extension| extension != "rs") {
    return Ok(0);
  }

  let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
  let formatted = rusty_core::format_source(&source);

  if source == formatted {
    return Ok(0);
  }

  if check {
    eprintln!("{} needs formatting", path.display());
  } else {
    fs::write(path, formatted).map_err(|error| error.to_string())?;
    eprintln!("formatted {}", path.display());
  }

  Ok(1)
}

fn is_ignored_directory(name: &std::ffi::OsStr) -> bool {
  matches!(
    name.to_str(),
    Some(".direnv" | ".git" | "target" | "node_modules")
  )
}
