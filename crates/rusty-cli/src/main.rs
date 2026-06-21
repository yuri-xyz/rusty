use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, ffi};

use anstyle::AnsiColor;
use clap::{Parser, Subcommand};
use rusty_core::{
  Diagnostic, DiagnosticSeverity, NO_INLINE_MODULES_RULE_ID, NO_INLINE_TESTS_RULE_ID,
};
use serde::Deserialize;

const CONFIG_FILE_NAME: &str = ".rusty.toml";

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
  let config = match RustyConfig::load(&paths) {
    Ok(config) => config,
    Err(error) => {
      eprintln!("{error}");

      return ExitCode::from(2);
    }
  };

  let mut had_error = false;
  let mut checked_files = 0;
  let mut reports = Vec::new();

  for path in paths {
    match check_path(&path, &config) {
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

fn check_path(path: &Path, config: &RustyConfig) -> Result<CheckResult, String> {
  if config.is_ignored(path) {
    return Ok(CheckResult::default());
  }

  if path.is_dir() {
    let mut result = CheckResult::default();

    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
      let entry = entry.map_err(|error| error.to_string())?;
      let path = entry.path();

      if config.is_ignored(&path) {
        continue;
      }

      result.append(check_path(&path, config)?);
    }

    return Ok(result);
  }

  if path.extension().is_none_or(|extension| extension != "rs") {
    return Ok(CheckResult::default());
  }

  let source = fs::read_to_string(path).map_err(|error| error.to_string())?;

  let reports = rusty_core::check_file(path, &source)
    .into_iter()
    .filter(|diagnostic| config.rule_enabled(diagnostic.rule_id))
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
  let config = match RustyConfig::load(&paths) {
    Ok(config) => config,
    Err(error) => {
      eprintln!("{error}");

      return ExitCode::from(2);
    }
  };

  let mut had_error = false;
  let mut formatted_count = 0;

  for path in paths {
    match format_path(&path, check, &config) {
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

fn format_path(path: &Path, check: bool, config: &RustyConfig) -> Result<usize, String> {
  if config.is_ignored(path) {
    return Ok(0);
  }

  if path.is_dir() {
    let mut count = 0;

    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
      let entry = entry.map_err(|error| error.to_string())?;
      let path = entry.path();

      if config.is_ignored(&path) {
        continue;
      }

      count += format_path(&path, check, config)?;
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

fn is_ignored_directory(name: &ffi::OsStr) -> bool {
  matches!(
    name.to_str(),
    Some(".direnv" | ".git" | "target" | "node_modules")
  )
}

fn is_ignored_path(path: &Path) -> bool {
  path.file_name().is_some_and(is_ignored_directory)
}

#[derive(Debug, Default)]
struct RustyConfig {
  current_dir: PathBuf,
  ignored_paths: Vec<PathBuf>,
  rules: RuleConfig,
}

impl RustyConfig {
  fn load(paths: &[PathBuf]) -> Result<Self, String> {
    let current_dir = env::current_dir().map_err(|error| error.to_string())?;

    let Some(config_path) = find_config_path(paths, &current_dir) else {
      return Ok(Self {
        current_dir,
        ignored_paths: Vec::new(),
        rules: RuleConfig::default(),
      });
    };

    let source = fs::read_to_string(&config_path)
      .map_err(|error| format!("{}: failed to read config: {error}", config_path.display()))?;

    let config: RawConfig = toml::from_str(&source)
      .map_err(|error| format!("{}: failed to parse config: {error}", config_path.display()))?;

    let config_dir = config_path
      .parent()
      .ok_or_else(|| format!("{}: config path has no parent", config_path.display()))?;

    let ignored_paths = config
      .ignore
      .into_iter()
      .map(|path| normalize_path(&config_dir.join(path)))
      .collect();

    Ok(Self {
      current_dir,
      ignored_paths,
      rules: RuleConfig::from_raw(&config.rules),
    })
  }

  fn is_ignored(&self, path: &Path) -> bool {
    if is_ignored_path(path) {
      return true;
    }

    let normalized_path = normalize_path(&self.current_dir.join(path));

    self
      .ignored_paths
      .iter()
      .any(|ignored_path| normalized_path.starts_with(ignored_path))
  }

  fn rule_enabled(&self, rule_id: &str) -> bool {
    match rule_id {
      NO_INLINE_TESTS_RULE_ID => self.rules.no_inline_tests,
      NO_INLINE_MODULES_RULE_ID => self.rules.no_inline_modules,
      _ => true,
    }
  }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawConfig {
  ignore: Vec<PathBuf>,
  rules: RawRuleConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawRuleConfig {
  #[serde(rename = "no-inline-tests")]
  no_inline_tests: Option<bool>,
  #[serde(rename = "no-inline-modules")]
  no_inline_modules: Option<bool>,
}

#[derive(Debug, Default)]
struct RuleConfig {
  no_inline_tests: bool,
  no_inline_modules: bool,
}

impl RuleConfig {
  fn from_raw(raw: &RawRuleConfig) -> Self {
    Self {
      no_inline_tests: raw.no_inline_tests.unwrap_or(false),
      no_inline_modules: raw.no_inline_modules.unwrap_or(false),
    }
  }
}

fn find_config_path(paths: &[PathBuf], current_dir: &Path) -> Option<PathBuf> {
  if let Some(path) = find_config_in_ancestors(current_dir) {
    return Some(path);
  }

  paths.iter().find_map(|path| {
    let absolute_path = normalize_path(&current_dir.join(path));

    let search_dir = if absolute_path.is_dir() {
      absolute_path
    } else {
      absolute_path.parent()?.to_path_buf()
    };

    find_config_in_ancestors(&search_dir)
  })
}

fn find_config_in_ancestors(start: &Path) -> Option<PathBuf> {
  start
    .ancestors()
    .map(|path| path.join(CONFIG_FILE_NAME))
    .find(|path| path.is_file())
}

fn normalize_path(path: &Path) -> PathBuf {
  let mut normalized = PathBuf::new();

  for component in path.components() {
    match component {
      std::path::Component::CurDir => {}
      std::path::Component::ParentDir => {
        normalized.pop();
      }
      component => normalized.push(component.as_os_str()),
    }
  }

  normalized
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
