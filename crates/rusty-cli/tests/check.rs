use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn check_reports_compact_colored_diagnostics_without_dot_prefix() {
  let temp_dir = temp_dir();

  fs::create_dir_all(temp_dir.join("src")).expect("test should create src directory");
  fs::write(
    temp_dir.join("src/lib.rs"),
    "#[test]\nfn inline_test() {}\n",
  )
  .expect("test should write fixture source");

  let output = Command::new(env!("CARGO_BIN_EXE_rusty"))
    .arg("check")
    .arg(".")
    .env("CLICOLOR_FORCE", "1")
    .current_dir(&temp_dir)
    .output()
    .expect("test should run rusty check");

  fs::remove_dir_all(&temp_dir).expect("test should remove temporary directory");

  assert!(!output.status.success());

  let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

  assert!(stderr.contains("src/lib.rs:1:1"));
  assert!(stderr.contains("\u{1b}[90msrc/lib.rs:1:1\u{1b}[0m"));
  assert!(stderr.contains("[err/no-inline-tests]"));
  assert!(!stderr.contains("./src/lib.rs"));
}

#[test]
fn check_reports_when_no_issues_are_found() {
  let temp_dir = temp_dir();

  fs::create_dir_all(temp_dir.join("src")).expect("test should create src directory");
  fs::write(
    temp_dir.join("src/lib.rs"),
    "pub fn value() -> u8 {\n  1\n}\n",
  )
  .expect("test should write fixture source");

  let output = Command::new(env!("CARGO_BIN_EXE_rusty"))
    .arg("check")
    .arg(".")
    .current_dir(&temp_dir)
    .output()
    .expect("test should run rusty check");

  fs::remove_dir_all(&temp_dir).expect("test should remove temporary directory");

  assert!(output.status.success());

  let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

  assert!(stderr.contains("no issues found in 1 files"));
}

fn temp_dir() -> std::path::PathBuf {
  std::env::temp_dir().join(format!(
    "rusty-cli-test-{}",
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system time should be after the Unix epoch")
      .as_nanos()
  ))
}
