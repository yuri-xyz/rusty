# Rusty Project Guide

Use this guide when working as an AI coding agent in this repository.

## Project Purpose

Rusty is an opinionated Rust source toolchain for project-local formatting and linting rules that go beyond `rustfmt` and Clippy. It is intended to complement the standard Rust tools, not replace them.

The project currently provides:

- `rusty check`: runs Rusty lint rules and reports diagnostics.
- `rusty format`: rewrites source using Rusty formatter rules.
- `rusty format --check`: reports files that would be changed by formatting.

Every rule has a stable kebab-case id, such as `max-function-lines` or `block-spacing`.

## Repository Layout

- `crates/rusty-core`: parser integration, rule definitions, diagnostics, and formatter edits.
- `crates/rusty-cli`: command-line interface and terminal reporting.
- `README.md`: user-facing documentation for rules and development commands.
- `rustfmt.toml`: two-space Rust formatting configuration.
- `flake.nix`: Nix package, app, and development shell.

## Development Commands

Use these from the repository root:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run --bin rusty -- check .
cargo run --bin rusty -- format --check .
```

To apply Rust formatting:

```sh
cargo fmt --all
```

To run the CLI manually:

```sh
cargo run --bin rusty -- check .
cargo run --bin rusty -- format .
```

Through the flake app:

```sh
nix run .# -- check .
```

## Coding Conventions

- Follow the existing two-space Rust style.
- Keep rule logic in `rusty-core`; keep terminal rendering and CLI traversal in `rusty-cli`.
- Prefer typed data over parsing rendered strings. Diagnostics should carry structured fields such as rule id, severity, message, line, and column.
- Use `ra_ap_syntax` APIs for Rust syntax inspection instead of ad hoc source text parsing when possible.
- Keep lints conservative and deterministic.
- Do not add broad suppression or override behavior unless the rule explicitly supports it.
- When adding a rule, define a stable rule id constant and add it to `RULES`.
- Add focused tests for new rule behavior in `rusty-core` or CLI behavior in `rusty-cli`.
- Put tests under a `tests/` directory. Do not add inline `#[test]` functions to source files.

## Diagnostic Reporting

Check diagnostics are rendered by the CLI in this shape:

```text
path/to/file.rs:line:column[severity/rule-id]: message
```

The bracketed marker is colored by severity:

- `error`: red
- `warning`: yellow

All current lint rules report as `error`. Keep `warning` available for future non-blocking diagnostics.

Diagnostics should be sorted by severity first, with errors before warnings, then by path, line, column, and rule id. Display paths should not include a redundant leading `./`.

## Current Rules

Lint rules:

- `no-block-comments`: block comments are not allowed.
- `no-unsafe`: unsafe blocks are not allowed; this rule is overrideable.
- `no-unwrap`: `.unwrap()` is not allowed; use `.expect("...")` with a descriptive reason.
- `no-todo-comments`: `TODO`, `FIXME`, and `XXX` comments are not allowed.
- `no-inline-tests`: `#[test]` functions must live under a `tests/` directory.
- `no-inline-modules`: module bodies must live in separate files.
- `max-function-args`: functions may have at most four explicit parameters.
- `max-function-lines`: function bodies may contain at most 80 code lines.
- `max-impl-lines`: impl blocks may contain at most 80 code lines.
- `max-nesting-depth`: control flow may be nested at most four levels deep.
- `max-struct-fields`: structs may have at most 12 fields; this rule is overrideable.
- `max-file-lines`: Rust source files may contain at most 700 code lines.

Formatter rules:

- `block-spacing`: inserts blank lines between groups of direct child constructs inside blocks.

For line-count rules, only Rust syntax tokens count. Blank lines and comment-only lines do not count.

## Implementation Notes

`rusty-core` parses source with `SourceFile::parse(source, Edition::CURRENT)`. Rule checks generally walk syntax nodes or tokens and return `Vec<Diagnostic>`.

Formatter behavior returns text edits and applies them from the end of the file toward the beginning to preserve byte offsets.

The CLI recursively walks input paths, ignores `.direnv`, `.git`, `target`, and `node_modules`, and only checks or formats `.rs` files.

## Before Finishing Changes

Run at least:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo run --bin rusty -- check .
```

Also run Clippy when changing implementation logic:

```sh
cargo clippy --workspace --all-targets
```
