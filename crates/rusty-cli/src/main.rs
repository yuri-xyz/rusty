use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anstyle::AnsiColor;
use clap::{Parser, Subcommand};
use rusty_core::{Diagnostic, DiagnosticSeverity};

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
  let mut checked_files = 0;
  let mut reports = Vec::new();

  for path in paths {
    match check_path(&path) {
      Ok(CheckResult {
        checked_file_count,
        reports: mut path_reports,
      }) => {
        checked_files += checked_file_count;
        reports.append(&mut path_reports);
      }
      Err(error) => {
        eprintln!("{}: {error}", path.display());
        had_error = true;
      }
    }
  }

  reports.sort_by(report_order);

  for report in &reports {
    print_diagnostic(report);
  }

  if !reports.is_empty() {
    return ExitCode::from(1);
  }

  if had_error {
    return ExitCode::from(2);
  }

  anstream::eprintln!("no issues found in {checked_files} files");

  ExitCode::SUCCESS
}

fn check_path(path: &Path) -> Result<CheckResult, String> {
  if path.is_dir() {
    let mut result = CheckResult::default();

    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
      let entry = entry.map_err(|error| error.to_string())?;
      let path = entry.path();

      if path.file_name().is_some_and(is_ignored_directory) {
        continue;
      }

      result.append(check_path(&path)?);
    }

    return Ok(result);
  }

  if path.extension().is_none_or(|extension| extension != "rs") {
    return Ok(CheckResult::default());
  }

  let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
  let reports = rusty_core::check_file(path, &source)
    .into_iter()
    .map(|diagnostic| DiagnosticReport {
      path: path.to_owned(),
      diagnostic,
    })
    .collect();

  Ok(CheckResult {
    checked_file_count: 1,
    reports,
  })
}

fn report_order(left: &DiagnosticReport, right: &DiagnosticReport) -> std::cmp::Ordering {
  (
    left.diagnostic.severity,
    &left.path,
    left.diagnostic.line,
    left.diagnostic.column,
    left.diagnostic.rule_id,
  )
    .cmp(&(
      right.diagnostic.severity,
      &right.path,
      right.diagnostic.line,
      right.diagnostic.column,
      right.diagnostic.rule_id,
    ))
}

fn print_diagnostic(report: &DiagnosticReport) {
  let diagnostic = &report.diagnostic;
  let location = diagnostic_location(report, true);
  let marker = diagnostic_marker(diagnostic, true);

  eprintln!("{}{}: {}", location, marker, diagnostic.message);
}

fn diagnostic_location(report: &DiagnosticReport, color: bool) -> String {
  let diagnostic = &report.diagnostic;
  let location = format!(
    "{}:{}:{}",
    display_path(&report.path).display(),
    diagnostic.line,
    diagnostic.column
  );

  if !color {
    return location;
  }

  let style = AnsiColor::BrightBlack.on_default();

  format!("{}{}{}", style.render(), location, style.render_reset())
}

fn diagnostic_marker(diagnostic: &Diagnostic, color: bool) -> String {
  let marker = format!(
    "[{}/{}]",
    severity_label(diagnostic.severity),
    diagnostic.rule_id
  );

  if !color {
    return marker;
  }

  let style = match diagnostic.severity {
    DiagnosticSeverity::Error => AnsiColor::Red.on_default().bold(),
    DiagnosticSeverity::Warning => AnsiColor::Yellow.on_default().bold(),
  };

  format!("{}{}{}", style.render(), marker, style.render_reset())
}

fn display_path(path: &Path) -> &Path {
  path.strip_prefix(".").unwrap_or(path)
}

fn severity_label(severity: DiagnosticSeverity) -> &'static str {
  match severity {
    DiagnosticSeverity::Error => "err",
    DiagnosticSeverity::Warning => "warn",
  }
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

#[derive(Debug)]
struct DiagnosticReport {
  path: PathBuf,
  diagnostic: Diagnostic,
}

#[derive(Debug, Default)]
struct CheckResult {
  checked_file_count: usize,
  reports: Vec<DiagnosticReport>,
}

impl CheckResult {
  fn append(&mut self, mut other: Self) {
    self.checked_file_count += other.checked_file_count;
    self.reports.append(&mut other.reports);
  }
}
