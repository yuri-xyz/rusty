use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn check_reports_compact_colored_diagnostics_without_dot_prefix() {
  let temp_dir = temp_dir();

  fs::create_dir_all(temp_dir.join("src")).unwrap();
  fs::write(
    temp_dir.join("src/lib.rs"),
    "#[test]\nfn inline_test() {}\n",
  )
  .unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_rusty"))
    .arg("check")
    .arg(".")
    .current_dir(&temp_dir)
    .output()
    .unwrap();

  fs::remove_dir_all(&temp_dir).unwrap();

  assert!(!output.status.success());

  let stderr = String::from_utf8(output.stderr).unwrap();

  assert!(stderr.contains("src/lib.rs:1:1"));
  assert!(stderr.contains("[error/no-inline-tests]"));
  assert!(!stderr.contains("./src/lib.rs"));
}

#[test]
fn check_reports_when_no_issues_are_found() {
  let temp_dir = temp_dir();

  fs::create_dir_all(temp_dir.join("src")).unwrap();
  fs::write(
    temp_dir.join("src/lib.rs"),
    "pub fn value() -> u8 {\n  1\n}\n",
  )
  .unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_rusty"))
    .arg("check")
    .arg(".")
    .current_dir(&temp_dir)
    .output()
    .unwrap();

  fs::remove_dir_all(&temp_dir).unwrap();

  assert!(output.status.success());

  let stderr = String::from_utf8(output.stderr).unwrap();

  assert!(stderr.contains("no issues found in 1 files"));
}

fn temp_dir() -> std::path::PathBuf {
  std::env::temp_dir().join(format!(
    "rusty-cli-test-{}",
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos()
  ))
}
