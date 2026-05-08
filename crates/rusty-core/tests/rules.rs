use std::fmt::Write;
use std::path::Path;

use rusty_core::{
  MAX_FILE_LINES_RULE_ID, MAX_FUNCTION_ARGS_RULE_ID, MAX_FUNCTION_LINES_RULE_ID,
  NO_BLOCK_COMMENTS_RULE_ID, NO_INLINE_TESTS_RULE_ID, check_file, check_source,
};

#[test]
fn reports_functions_with_more_than_four_explicit_parameters() {
  let source = r"
fn allowed(a: u8, b: u8, c: u8, d: u8) {}

fn denied(a: u8, b: u8, c: u8, d: u8, e: u8) {}
";

  let diagnostics = check_source(source);

  assert_eq!(diagnostics.len(), 1);
  assert_eq!(diagnostics[0].rule_id, MAX_FUNCTION_ARGS_RULE_ID);

  assert!(
    diagnostics[0]
      .message
      .contains("function `denied` has 5 parameters")
  );
}

#[test]
fn ignores_method_receiver_when_counting_parameters() {
  let source = r"
struct Service;

impl Service {
  fn allowed(&self, a: u8, b: u8, c: u8, d: u8) {}
}
";

  let diagnostics = check_source(source);

  assert!(diagnostics.is_empty());
}

#[test]
fn reports_functions_with_more_than_eighty_code_lines() {
  let mut source = String::from("fn too_long() {\n");

  for line in 0..81 {
    writeln!(source, "  let value_{line} = {line};").unwrap();
  }

  source.push_str("}\n");

  let diagnostics = check_source(&source);

  assert_eq!(diagnostics.len(), 1);
  assert_eq!(diagnostics[0].rule_id, MAX_FUNCTION_LINES_RULE_ID);

  assert!(
    diagnostics[0]
      .message
      .contains("function `too_long` has 81 lines of code")
  );
}

#[test]
fn ignores_blank_and_comment_only_lines_for_function_line_count() {
  let mut source = String::from("fn allowed() {\n");

  for line in 0..80 {
    writeln!(source, "  let value_{line} = {line};").unwrap();
  }

  for _ in 0..20 {
    source.push_str("\n  // comment\n");
  }

  source.push_str("}\n");

  let diagnostics = check_source(&source);

  assert!(diagnostics.is_empty());
}

#[test]
fn reports_files_with_more_than_seven_hundred_code_lines() {
  let mut source = String::new();

  for line in 0..701 {
    writeln!(source, "const VALUE_{line}: usize = {line};").unwrap();
  }

  let diagnostics = check_source(&source);

  assert_eq!(diagnostics.len(), 1);
  assert_eq!(diagnostics[0].rule_id, MAX_FILE_LINES_RULE_ID);
  assert!(diagnostics[0].message.contains("this file has 701"));

  assert!(
    diagnostics[0]
      .message
      .contains("Removing comments or blank lines does not count")
  );
}

#[test]
fn ignores_blank_and_comment_only_lines_for_file_line_count() {
  let source = r"
// comment

// more comments
/// doc comment
fn allowed() {}
";

  let diagnostics = check_source(source);

  assert!(diagnostics.is_empty());
}

#[test]
fn reports_block_comments() {
  let source = r"
fn main() {
  /* prefer line comments */
  let value = 5;
}
";

  let diagnostics = check_source(source);

  assert_eq!(diagnostics.len(), 1);
  assert_eq!(diagnostics[0].rule_id, NO_BLOCK_COMMENTS_RULE_ID);
  assert!(diagnostics[0].message.contains("use `//` or `///`"));
}

#[test]
fn allows_line_and_doc_line_comments() {
  let source = r"
/// Adds two values.
fn add(left: i32, right: i32) -> i32 {
  // Keep this obvious.
  left + right
}
";

  let diagnostics = check_source(source);

  assert!(diagnostics.is_empty());
}

#[test]
fn reports_inline_tests_outside_tests_directory() {
  let source = r"
#[test]
fn adds_values() {}
";

  let diagnostics = check_file(Path::new("src/lib.rs"), source);

  assert_eq!(diagnostics.len(), 1);
  assert_eq!(diagnostics[0].rule_id, NO_INLINE_TESTS_RULE_ID);
  assert!(
    diagnostics[0]
      .message
      .contains("test function `adds_values`")
  );
}

#[test]
fn allows_test_functions_under_tests_directory() {
  let source = r"
#[test]
fn adds_values() {}
";

  let diagnostics = check_file(Path::new("tests/rules.rs"), source);

  assert!(diagnostics.is_empty());
}
