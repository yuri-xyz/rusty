use std::collections::BTreeSet;
use std::path::Path;

use ra_ap_syntax::{
  AstNode, AstToken, Edition, NodeOrToken, SourceFile, SyntaxKind, TextRange,
  ast::{self, CommentShape, HasName},
};

pub const NO_BLOCK_COMMENTS_RULE_ID: &str = "no-block-comments";
pub const MAX_FUNCTION_ARGS_RULE_ID: &str = "max-function-args";
pub const MAX_FUNCTION_LINES_RULE_ID: &str = "max-function-lines";
pub const MAX_FILE_LINES_RULE_ID: &str = "max-file-lines";
pub const BLOCK_SPACING_RULE_ID: &str = "block-spacing";

const MAX_FUNCTION_ARGS: usize = 4;
const MAX_FUNCTION_CODE_LINES: usize = 80;
const MAX_FILE_CODE_LINES: usize = 700;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
  Lint,
  Formatter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
  pub id: &'static str,
  pub kind: RuleKind,
  pub can_override: bool,
}

pub const RULES: &[Rule] = &[
  Rule {
    id: NO_BLOCK_COMMENTS_RULE_ID,
    kind: RuleKind::Lint,
    can_override: false,
  },
  Rule {
    id: MAX_FUNCTION_ARGS_RULE_ID,
    kind: RuleKind::Lint,
    can_override: false,
  },
  Rule {
    id: MAX_FUNCTION_LINES_RULE_ID,
    kind: RuleKind::Lint,
    can_override: false,
  },
  Rule {
    id: MAX_FILE_LINES_RULE_ID,
    kind: RuleKind::Lint,
    can_override: false,
  },
  Rule {
    id: BLOCK_SPACING_RULE_ID,
    kind: RuleKind::Formatter,
    can_override: false,
  },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
  pub rule_id: &'static str,
  pub message: String,
  pub line: usize,
  pub column: usize,
}

#[must_use]
pub fn check_file(path: &Path, source: &str) -> Vec<Diagnostic> {
  let mut diagnostics = Vec::new();

  if path.extension().is_some_and(|extension| extension == "rs") {
    diagnostics.extend(check_source(source));
  }

  diagnostics
}

#[must_use]
pub fn check_source(source: &str) -> Vec<Diagnostic> {
  let parse = SourceFile::parse(source, Edition::CURRENT);
  let file = parse.tree();
  let line_index = LineIndex::new(source);

  let mut diagnostics = Vec::new();

  diagnostics.extend(check_no_block_comments(&file, &line_index));
  diagnostics.extend(check_max_function_args(&file, &line_index));
  diagnostics.extend(check_max_function_lines(&file, &line_index));
  diagnostics.extend(check_max_file_lines(&file));

  diagnostics
}

#[must_use]
pub fn format_source(source: &str) -> String {
  let parse = SourceFile::parse(source, Edition::CURRENT);
  let file = parse.tree();
  let mut edits = block_spacing_edits(source, &file);

  if edits.is_empty() {
    return source.to_owned();
  }

  edits.sort_by_key(|edit| edit.start);

  let mut formatted = source.to_owned();

  for edit in edits.into_iter().rev() {
    formatted.replace_range(edit.start..edit.end, &edit.replacement);
  }

  formatted
}

fn check_no_block_comments(file: &SourceFile, line_index: &LineIndex) -> Vec<Diagnostic> {
  file
    .syntax()
    .descendants_with_tokens()
    .filter_map(NodeOrToken::into_token)
    .filter_map(ast::Comment::cast)
    .filter_map(|comment| {
      if comment.kind().shape != CommentShape::Block {
        return None;
      }

      let (line, column) = line_index.position(comment.syntax().text_range().start());

      Some(Diagnostic {
        rule_id: NO_BLOCK_COMMENTS_RULE_ID,
        message: "block comments are not allowed; use `//` or `///` line comments instead"
          .to_owned(),
        line,
        column,
      })
    })
    .collect()
}

fn check_max_function_args(file: &SourceFile, line_index: &LineIndex) -> Vec<Diagnostic> {
  file
    .syntax()
    .descendants()
    .filter_map(ast::Fn::cast)
    .filter_map(|function| {
      let param_list = function.param_list()?;
      let arg_count = param_list.params().count();

      if arg_count <= MAX_FUNCTION_ARGS {
        return None;
      }

      let (line, column) = line_index.position(function.syntax().text_range().start());

      let name = function.name().map_or_else(
        || "function".to_owned(),
        |name| format!("function `{name}`"),
      );

      Some(Diagnostic {
        rule_id: MAX_FUNCTION_ARGS_RULE_ID,
        message: format!(
          "{name} has {arg_count} parameters; functions must have at most \
           {MAX_FUNCTION_ARGS}. Use a record/struct to group related inputs."
        ),
        line,
        column,
      })
    })
    .collect()
}

fn check_max_function_lines(file: &SourceFile, line_index: &LineIndex) -> Vec<Diagnostic> {
  file
    .syntax()
    .descendants()
    .filter_map(ast::Fn::cast)
    .filter_map(|function| {
      let stmt_list = function
        .syntax()
        .children()
        .find_map(ast::BlockExpr::cast)?
        .stmt_list()?;

      let code_lines = code_line_numbers_for_node(line_index, stmt_list.syntax()).len();

      if code_lines <= MAX_FUNCTION_CODE_LINES {
        return None;
      }

      let (line, column) = line_index.position(function.syntax().text_range().start());

      let name = function.name().map_or_else(
        || "function".to_owned(),
        |name| format!("function `{name}`"),
      );

      Some(Diagnostic {
        rule_id: MAX_FUNCTION_LINES_RULE_ID,
        message: format!(
          "{name} has {code_lines} lines of code; functions must have at most \
           {MAX_FUNCTION_CODE_LINES}. Split complex logic into smaller functions."
        ),
        line,
        column,
      })
    })
    .collect()
}

fn check_max_file_lines(file: &SourceFile) -> Vec<Diagnostic> {
  let code_lines = code_line_numbers(file);

  if code_lines.len() <= MAX_FILE_CODE_LINES {
    return Vec::new();
  }

  vec![Diagnostic {
    rule_id: MAX_FILE_LINES_RULE_ID,
    message: format!(
      "Rust source files must contain at most {MAX_FILE_CODE_LINES} lines of code; this file has \
       {}. Split the file into smaller modules. Removing comments or blank lines does not count \
       and should not be used to circumvent this rule.",
      code_lines.len()
    ),
    line: 1,
    column: 1,
  }]
}

fn code_line_numbers(file: &SourceFile) -> BTreeSet<usize> {
  let text = file.syntax().text().to_string();
  let line_index = LineIndex::new(&text);

  code_line_numbers_for_node(&line_index, file.syntax())
}

fn code_line_numbers_for_node(
  line_index: &LineIndex,
  node: &ra_ap_syntax::SyntaxNode,
) -> BTreeSet<usize> {
  let mut code_lines = BTreeSet::new();

  for element in node.descendants_with_tokens() {
    let NodeOrToken::Token(token) = element else {
      continue;
    };

    if is_non_code_token(token.kind()) {
      continue;
    }

    let range = token.text_range();

    for line in line_index.lines_for_range(range) {
      code_lines.insert(line);
    }
  }

  code_lines
}

fn is_non_code_token(kind: SyntaxKind) -> bool {
  matches!(
    kind,
    SyntaxKind::WHITESPACE | SyntaxKind::COMMENT | SyntaxKind::L_CURLY | SyntaxKind::R_CURLY
  )
}

fn block_spacing_edits(source: &str, file: &SourceFile) -> Vec<TextEdit> {
  let mut edits = Vec::new();

  for stmt_list in file.syntax().descendants().filter_map(ast::StmtList::cast) {
    let entries = block_entries(source, &stmt_list);

    for pair in entries.windows(2) {
      let previous = pair[0];
      let next = pair[1];

      if previous.kind == next.kind && !previous.is_multiline && !next.is_multiline {
        continue;
      }

      let start = usize::from(previous.range.end());
      let end = usize::from(next.range.start());

      if start > end || !source[start..end].chars().all(char::is_whitespace) {
        continue;
      }

      let indentation = trailing_indentation(&source[start..end]);
      let replacement = format!("\n\n{indentation}");

      if source[start..end] != replacement {
        edits.push(TextEdit {
          start,
          end,
          replacement,
        });
      }
    }
  }

  edits
}

fn block_entries(source: &str, stmt_list: &ast::StmtList) -> Vec<BlockEntry> {
  let mut entries = stmt_list
    .statements()
    .filter_map(|stmt| {
      let kind = statement_spacing_kind(&stmt)?;
      let range = stmt.syntax().text_range();

      Some(BlockEntry {
        range,
        kind,
        is_multiline: range_is_multiline(source, range),
      })
    })
    .collect::<Vec<_>>();

  if let Some(tail_expr) = stmt_list.tail_expr() {
    let range = tail_expr.syntax().text_range();

    entries.push(BlockEntry {
      range,
      kind: SpacingKind::Yield,
      is_multiline: range_is_multiline(source, range),
    });
  }

  entries.sort_by_key(|entry| entry.range.start());
  entries.dedup_by_key(|entry| entry.range);

  entries
}

fn range_is_multiline(source: &str, range: TextRange) -> bool {
  let start = usize::from(range.start());
  let end = usize::from(range.end());

  source[start..end].contains('\n')
}

fn statement_spacing_kind(stmt: &ast::Stmt) -> Option<SpacingKind> {
  match stmt {
    ast::Stmt::LetStmt(_) => Some(SpacingKind::Declaration),
    ast::Stmt::Item(item) => Some(item_spacing_kind(item)),
    ast::Stmt::ExprStmt(stmt) => Some(expr_spacing_kind(&stmt.expr()?)),
  }
}

fn item_spacing_kind(item: &ast::Item) -> SpacingKind {
  match item {
    ast::Item::AsmExpr(_) | ast::Item::MacroCall(_) => SpacingKind::Action,
    ast::Item::Const(_)
    | ast::Item::Enum(_)
    | ast::Item::ExternBlock(_)
    | ast::Item::ExternCrate(_)
    | ast::Item::Fn(_)
    | ast::Item::Impl(_)
    | ast::Item::MacroDef(_)
    | ast::Item::MacroRules(_)
    | ast::Item::Module(_)
    | ast::Item::Static(_)
    | ast::Item::Struct(_)
    | ast::Item::Trait(_)
    | ast::Item::TypeAlias(_)
    | ast::Item::Union(_)
    | ast::Item::Use(_) => SpacingKind::Declaration,
  }
}

fn expr_spacing_kind(expr: &ast::Expr) -> SpacingKind {
  match expr {
    ast::Expr::ReturnExpr(_) | ast::Expr::BecomeExpr(_) | ast::Expr::YeetExpr(_) => {
      SpacingKind::Yield
    }
    ast::Expr::BreakExpr(_)
    | ast::Expr::ContinueExpr(_)
    | ast::Expr::ForExpr(_)
    | ast::Expr::LoopExpr(_)
    | ast::Expr::WhileExpr(_)
    | ast::Expr::YieldExpr(_) => SpacingKind::Control,
    ast::Expr::IfExpr(_) => SpacingKind::Conditional,
    ast::Expr::MatchExpr(_) => SpacingKind::Match,
    ast::Expr::AwaitExpr(_)
    | ast::Expr::ArrayExpr(_)
    | ast::Expr::AsmExpr(_)
    | ast::Expr::BinExpr(_)
    | ast::Expr::BlockExpr(_)
    | ast::Expr::CallExpr(_)
    | ast::Expr::CastExpr(_)
    | ast::Expr::ClosureExpr(_)
    | ast::Expr::FieldExpr(_)
    | ast::Expr::FormatArgsExpr(_)
    | ast::Expr::IndexExpr(_)
    | ast::Expr::LetExpr(_)
    | ast::Expr::Literal(_)
    | ast::Expr::MacroExpr(_)
    | ast::Expr::MethodCallExpr(_)
    | ast::Expr::OffsetOfExpr(_)
    | ast::Expr::ParenExpr(_)
    | ast::Expr::PathExpr(_)
    | ast::Expr::PrefixExpr(_)
    | ast::Expr::RangeExpr(_)
    | ast::Expr::RecordExpr(_)
    | ast::Expr::RefExpr(_)
    | ast::Expr::TryExpr(_)
    | ast::Expr::TupleExpr(_)
    | ast::Expr::UnderscoreExpr(_) => SpacingKind::Action,
  }
}

fn trailing_indentation(whitespace: &str) -> &str {
  whitespace
    .rsplit_once('\n')
    .map_or(whitespace, |(_, indentation)| indentation)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockEntry {
  range: TextRange,
  kind: SpacingKind,
  is_multiline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpacingKind {
  Declaration,
  Action,
  Conditional,
  Match,
  Control,
  Yield,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextEdit {
  start: usize,
  end: usize,
  replacement: String,
}

#[derive(Debug)]
struct LineIndex {
  starts: Vec<usize>,
}

impl LineIndex {
  fn new(source: &str) -> Self {
    let mut starts = vec![0];

    for (index, byte) in source.bytes().enumerate() {
      if byte == b'\n' {
        starts.push(index + 1);
      }
    }

    Self { starts }
  }

  fn position(&self, offset: ra_ap_syntax::TextSize) -> (usize, usize) {
    let offset = usize::from(offset);
    let line_index = self.line_index(offset);
    let line_start = self.starts[line_index];

    (line_index + 1, offset - line_start + 1)
  }

  fn lines_for_range(&self, range: TextRange) -> impl Iterator<Item = usize> {
    let start = usize::from(range.start());
    let end = usize::from(range.end()).saturating_sub(1);
    let start_line = self.line_index(start);
    let end_line = self.line_index(end);

    (start_line + 1)..=(end_line + 1)
  }

  fn line_index(&self, offset: usize) -> usize {
    match self.starts.binary_search(&offset) {
      Ok(line) => line,
      Err(next_line) => next_line.saturating_sub(1),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fmt::Write;

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
      writeln!(source, "const VALUE_{line}: usize = {line}").unwrap();
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

    assert_eq!(
      code_line_numbers(&SourceFile::parse(source, Edition::CURRENT).tree()).len(),
      1
    );
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
  fn formats_construct_groups_inside_blocks() {
    let source = r"
fn main() {
  let foo = value();
  let bar = value();
  foo::bar();
  foo.bar();
  return 5;
}
";

    let expected = r"
fn main() {
  let foo = value();
  let bar = value();

  foo::bar();
  foo.bar();

  return 5;
}
";

    assert_eq!(format_source(source), expected);
  }

  #[test]
  fn formats_tail_expression_as_a_separate_group() {
    let source = r"
fn value() -> i32 {
  let foo = 5;
  foo
}
";

    let expected = r"
fn value() -> i32 {
  let foo = 5;

  foo
}
";

    assert_eq!(format_source(source), expected);
  }

  #[test]
  fn formats_nested_block_construct_groups() {
    let source = r"
fn main() {
  if enabled {
    let foo = 5;
    work(foo);
  }
}
";

    let expected = r"
fn main() {
  if enabled {
    let foo = 5;

    work(foo);
  }
}
";

    assert_eq!(format_source(source), expected);
  }

  #[test]
  fn leaves_comment_separated_constructs_unchanged() {
    let source = r"
fn main() {
  let foo = 5;
  // Keep this attached to the call.
  work(foo);
}
";

    assert_eq!(format_source(source), source);
  }

  #[test]
  fn formats_conditionals_and_match_as_separate_groups() {
    let source = r"
fn main() {
  let value = read();
  call(value);
  if value > 5 {
    call(value);
  } else if value > 2 {
    other(value);
  } else {
    fallback();
  }
  match value {
    0 => zero(),
    _ => many(),
  }
  finish();
}
";

    let expected = r"
fn main() {
  let value = read();

  call(value);

  if value > 5 {
    call(value);
  } else if value > 2 {
    other(value);
  } else {
    fallback();
  }

  match value {
    0 => zero(),
    _ => many(),
  }

  finish();
}
";

    assert_eq!(format_source(source), expected);
  }

  #[test]
  fn keeps_macro_calls_with_action_group() {
    let source = r"
fn main() {
  let value = read();
  trace!(value);
  call(value);
}
";

    let expected = r"
fn main() {
  let value = read();

  trace!(value);
  call(value);
}
";

    assert_eq!(format_source(source), expected);
  }

  #[test]
  fn separates_multiline_entries_even_when_kind_matches() {
    let source = r"
fn main() {
  let foo = {
    value()
  };
  let bar = value();
  let baz = value();
  call(
    foo,
  );
  other(foo);
  let record = Config {
    foo,
    bar,
  };
  let final_value = value();
}
";

    let expected = r"
fn main() {
  let foo = {
    value()
  };

  let bar = value();
  let baz = value();

  call(
    foo,
  );

  other(foo);

  let record = Config {
    foo,
    bar,
  };

  let final_value = value();
}
";

    assert_eq!(format_source(source), expected);
  }

  #[test]
  fn separates_adjacent_multiline_matches() {
    let source = r"
fn main() {
  match first {
    Some(value) => value,
    None => 0,
  }
  match second {
    Some(value) => value,
    None => 0,
  }
}
";

    let expected = r"
fn main() {
  match first {
    Some(value) => value,
    None => 0,
  }

  match second {
    Some(value) => value,
    None => 0,
  }
}
";

    assert_eq!(format_source(source), expected);
  }
}
