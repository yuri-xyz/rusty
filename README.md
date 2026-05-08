# Rusty

Rusty is an opinionated Rust source toolchain for project-local formatting and linting rules that go beyond `rustfmt` and Clippy.

The goal is to make custom Rust style rules explicit, standardized, and enforceable in the same spirit as ESLint:

- each rule is isolated;
- each rule has a stable name/id;
- rules can report diagnostics with precise source locations;
- rules can optionally provide fixes or formatting edits;
- rule overrides are standardized and auditable.

Rusty is intended to complement the official Rust tools, not replace them. A typical project would still run `rustfmt`, `clippy`, and `cargo test`, then run Rusty for rules that are specific to the project's safety, readability, or style policy.

## Rules

Every lint or formatting rule has a stable id. Rule ids use kebab-case names such as:

```text
no-unsafe
prefer-multiline-builder-chain
module-order
```

Rules are implemented independently so they can be enabled, disabled, tested, documented, and evolved without coupling unrelated checks together.

A rule should define:

- its id;
- whether it is a lint, formatter, or both;
- what syntax it inspects;
- what diagnostic it emits;
- whether it can be overridden;
- whether it can provide an automatic fix.

## Overrides

Rusty supports source-level overrides using Rust comments.

The standard override format is:

```rust
// rusty::override(<rule-id>): <explanation>
```

For example:

```rust
// rusty::override(no-unsafe): required for FFI call; pointer validity is checked by the caller.
unsafe {
  ffi_call(ptr);
}
```

Overrides are opt-in per rule. A rule can only be overridden if that rule explicitly declares that overrides are allowed. This prevents broad comment-based suppression from becoming the default escape hatch.

For example, `no-unsafe` is expected to be overrideable because there are legitimate cases where `unsafe` is necessary, but every exception should carry a local explanation. Other rules may choose to reject overrides entirely.

An override must include an explanation. Empty or placeholder explanations should be treated as invalid.

## Initial Rule: `no-unsafe`

The first target lint is `no-unsafe`.

`no-unsafe` reports usage of `unsafe` unless it is immediately documented with a valid Rusty override comment:

```rust
// rusty::override(no-unsafe): explains why this unsafe block is required and sound.
unsafe {
  // ...
}
```

Invalid examples:

```rust
unsafe {
  // ...
}
```

```rust
// rusty::override(no-unsafe):
unsafe {
  // ...
}
```

```rust
// rusty::override(some-other-rule): this does not override no-unsafe.
unsafe {
  // ...
}
```

## Implemented Rules

### `no-block-comments`

Block comments are not allowed.

Use line comments instead:

```rust
// regular comments
/// public documentation
//! module documentation
```

Invalid examples:

```rust
/* block comment */
/** block doc comment */
/*! inner block doc comment */
```

This rule is not overrideable.

### `max-function-args`

Functions may have at most four explicit parameters.

When a function needs more than four inputs, group related inputs into a named record/struct instead of extending the parameter list.

Method receivers such as `self`, `&self`, and `&mut self` are not counted as explicit parameters.

This rule is not overrideable.

### `max-function-lines`

Function bodies may contain at most 80 lines of code.

Only lines containing Rust syntax tokens inside the function body are counted. Blank lines and comment-only lines do not count toward the limit, so removing comments or whitespace is not a valid way to satisfy the rule. Functions that exceed the limit should be split into smaller helpers with clearer responsibilities.

This rule is not overrideable.

### `max-file-lines`

Rust source files may contain at most 700 lines of code.

Only lines containing Rust syntax tokens are counted. Blank lines and comment-only lines do not count toward the limit, so deleting comments or whitespace is not a valid way to satisfy the rule. Files that exceed the limit should be split into smaller modules.

This rule is not overrideable.

### `block-spacing`

Nested Rust blocks should group direct child constructs by kind with one blank line between groups.

Any multi-line construct is also separated from adjacent constructs, even when the neighboring construct has the same kind. This keeps larger expressions, records, calls, conditionals, and matches visually isolated from surrounding code.

Rusty currently separates these groups:

- declarations, such as `let`, `const`, `static`, local items, and `use`;
- actions, such as function calls, method calls, field access, indexing, macros, and other expression statements;
- conditionals, such as `if`, `else`, and `else if` chains;
- matches, such as `match`;
- other control flow, such as loops, `break`, and `continue`;
- final values, such as `return` and tail expressions.

For example:

```rust
{
  let bar = value();
  const FOO: usize = 1;
  let baz = other();

  something::do_something();
  abc::zdf::ald();

  return 5;
}
```

Multi-line constructs are isolated:

```rust
{
  let foo = {
    value()
  };

  let bar = value();
  let baz = value();

  call(
    foo,
  );

  other(foo);
}
```

The formatter is conservative around comments. It only rewrites whitespace-only gaps between adjacent constructs, so comments are not moved or detached from nearby code.

This rule is not overrideable.

## Workspace

This repository is a Cargo workspace:

```text
crates/
  rusty-core/
    Core rule definitions, parser integration, diagnostics, and text edits.
  rusty-cli/
    Command-line interface for checking and fixing Rust source files.
```

Formatting for this repository is configured in `rustfmt.toml`. The project uses two-space indentation:

```toml
tab_spaces = 2
```

## Development

Enter the development shell:

```sh
nix --extra-experimental-features 'nix-command flakes' develop
```

Or enable direnv for automatic shell activation:

```sh
direnv allow
```

Run formatting checks:

```sh
cargo fmt --all -- --check
```

Run compile checks:

```sh
cargo check --workspace
```

Run Rusty against the current project:

```sh
cargo run --bin rusty -- check .
```

Format the current project with Rusty formatter rules:

```sh
cargo run --bin rusty -- format .
```

Check Rusty formatting without writing changes:

```sh
cargo run --bin rusty -- format --check .
```

When no command is provided, the CLI defaults to checking the current directory:

```sh
cargo run --bin rusty
```

Run through the flake app:

```sh
nix run .# -- check .
```

Install from a NixOS flake by adding Rusty as an input and including the package:

```nix
{
  inputs.rusty.url = "github:<owner>/rusty";

  outputs =
    { nixpkgs, rusty, ... }:
    {
      nixosConfigurations.<hostname> = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          {
            environment.systemPackages = [
              rusty.packages.x86_64-linux.default
            ];
          }
        ];
      };
    };
}
```
