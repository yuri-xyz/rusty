use std::path::{Component, Path};

use ra_ap_syntax::{
  AstNode, AstToken, Edition, NodeOrToken, SourceFile, SyntaxKind, TextRange,
  ast::{self, CommentShape, HasAttrs, HasName},
};

pub const NO_BLOCK_COMMENTS_RULE_ID: &str = "no-block-comments";
pub const NO_INLINE_TESTS_RULE_ID: &str = "no-inline-tests";
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
    id: NO_INLINE_TESTS_RULE_ID,
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
  pub severity: DiagnosticSeverity,
  pub message: String,
  pub line: usize,
  pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticSeverity {
  Error,
  Warning,
}

#[must_use]
pub fn check_file(path: &Path, source: &str) -> Vec<Diagnostic> {
  let mut diagnostics = Vec::new();

  if path.extension().is_some_and(|extension| extension == "rs") {
    diagnostics.extend(check_source(source));

    if !path_is_in_tests_directory(path) {
      let parse = SourceFile::parse(source, Edition::CURRENT);
      let file = parse.tree();
      let line_index = LineIndex::new(source);

      diagnostics.extend(check_no_inline_tests(&file, &line_index));
    }
  }

  diagnostics
}

#[must_use]
pub fn check_source(source: &str) -> Vec<Diagnostic> {
  let parse = SourceFile::parse(source, Edition::CURRENT);
  let file = parse.tree();
  let line_index = LineIndex::new(source);
  let code_line_index = CodeLineIndex::new(&file, &line_index);

  let mut diagnostics = Vec::new();

  diagnostics.extend(check_no_block_comments(&file, &line_index));
  diagnostics.extend(check_max_function_args(&file, &line_index));
  diagnostics.extend(check_max_function_lines(&file, &line_index));
  diagnostics.extend(check_max_file_lines(&code_line_index));

  diagnostics
}

#[must_use]
pub fn format_source(source: &str) -> String {
  let parse = SourceFile::parse(source, Edition::CURRENT);
  let file = parse.tree();
  let line_index = LineIndex::new(source);
  let mut edits = block_spacing_edits(source, &file, &line_index);

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
        severity: DiagnosticSeverity::Error,
        message: "block comments are not allowed; use `//` or `///` line comments instead"
          .to_owned(),
        line,
        column,
      })
    })
    .collect()
}

fn check_no_inline_tests(file: &SourceFile, line_index: &LineIndex) -> Vec<Diagnostic> {
  file
    .syntax()
    .descendants()
    .filter_map(ast::Fn::cast)
    .filter(|function| function.attrs().any(|attr| attr_is_test(&attr)))
    .map(|function| {
      let (line, column) = line_index.position(function.syntax().text_range().start());
      let name = function.name().map_or_else(
        || "test function".to_owned(),
        |name| format!("test function `{name}`"),
      );

      Diagnostic {
        rule_id: NO_INLINE_TESTS_RULE_ID,
        severity: DiagnosticSeverity::Error,
        message: format!("{name} is inline; move tests into a `tests/` directory instead."),
        line,
        column,
      }
    })
    .collect()
}

fn attr_is_test(attr: &ast::Attr) -> bool {
  attr.simple_name().is_some_and(|name| name == "test")
}

fn path_is_in_tests_directory(path: &Path) -> bool {
  path.components().any(|component| {
    matches!(
      component,
      Component::Normal(name) if name == std::ffi::OsStr::new("tests")
    )
  })
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
        severity: DiagnosticSeverity::Error,
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

      let code_lines = count_code_lines_for_node(line_index, stmt_list.syntax());

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
        severity: DiagnosticSeverity::Error,
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

fn check_max_file_lines(code_line_index: &CodeLineIndex) -> Vec<Diagnostic> {
  let code_lines = code_line_index.total();

  if code_lines <= MAX_FILE_CODE_LINES {
    return Vec::new();
  }

  vec![Diagnostic {
    rule_id: MAX_FILE_LINES_RULE_ID,
    severity: DiagnosticSeverity::Error,
    message: format!(
      "Rust source files must contain at most {MAX_FILE_CODE_LINES} lines of code; this file has {code_lines}. \
       Split the file into smaller modules. Removing comments or blank lines does not count \
       and should not be used to circumvent this rule.",
    ),
    line: 1,
    column: 1,
  }]
}

fn is_non_code_token(kind: SyntaxKind) -> bool {
  matches!(
    kind,
    SyntaxKind::WHITESPACE | SyntaxKind::COMMENT | SyntaxKind::L_CURLY | SyntaxKind::R_CURLY
  )
}

fn count_code_lines_for_node(line_index: &LineIndex, node: &ra_ap_syntax::SyntaxNode) -> usize {
  let mut count = 0;
  let mut last_line = None;

  for element in node.descendants_with_tokens() {
    let NodeOrToken::Token(token) = element else {
      continue;
    };

    if is_non_code_token(token.kind()) {
      continue;
    }

    for line in line_index.lines_for_range(token.text_range()) {
      if last_line == Some(line) {
        continue;
      }

      count += 1;
      last_line = Some(line);
    }
  }

  count
}

fn block_spacing_edits(source: &str, file: &SourceFile, line_index: &LineIndex) -> Vec<TextEdit> {
  let mut edits = Vec::new();

  for stmt_list in file.syntax().descendants().filter_map(ast::StmtList::cast) {
    let entries = block_entries(line_index, &stmt_list);

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

fn block_entries(line_index: &LineIndex, stmt_list: &ast::StmtList) -> Vec<BlockEntry> {
  let mut entries = stmt_list
    .statements()
    .filter_map(|stmt| {
      let kind = statement_spacing_kind(&stmt)?;
      let range = stmt.syntax().text_range();

      Some(BlockEntry {
        range,
        kind,
        is_multiline: line_index.range_is_multiline(range),
      })
    })
    .collect::<Vec<_>>();

  if let Some(tail_expr) = stmt_list.tail_expr() {
    let range = tail_expr.syntax().text_range();
    let is_duplicate = entries.last().is_some_and(|entry| entry.range == range);

    if !is_duplicate {
      entries.push(BlockEntry {
        range,
        kind: SpacingKind::Yield,
        is_multiline: line_index.range_is_multiline(range),
      });
    }
  }

  entries
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
struct CodeLineIndex {
  prefix: Vec<usize>,
}

impl CodeLineIndex {
  fn new(file: &SourceFile, line_index: &LineIndex) -> Self {
    let mut has_code = vec![false; line_index.len()];

    for element in file.syntax().descendants_with_tokens() {
      let NodeOrToken::Token(token) = element else {
        continue;
      };

      if is_non_code_token(token.kind()) {
        continue;
      }

      for line in line_index.lines_for_range(token.text_range()) {
        has_code[line - 1] = true;
      }
    }

    let mut prefix = Vec::with_capacity(has_code.len() + 1);

    prefix.push(0);

    for has_code in has_code {
      let previous = *prefix.last().expect("prefix starts with a sentinel value");

      prefix.push(previous + usize::from(has_code));
    }

    Self { prefix }
  }

  fn total(&self) -> usize {
    *self
      .prefix
      .last()
      .expect("prefix starts with a sentinel value")
  }
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

  fn len(&self) -> usize {
    self.starts.len()
  }

  fn range_is_multiline(&self, range: TextRange) -> bool {
    let start = usize::from(range.start());
    let end = usize::from(range.end()).saturating_sub(1);

    self.line_index(start) != self.line_index(end)
  }

  fn line_index(&self, offset: usize) -> usize {
    match self.starts.binary_search(&offset) {
      Ok(line) => line,
      Err(next_line) => next_line.saturating_sub(1),
    }
  }
}
