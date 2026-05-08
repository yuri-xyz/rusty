# Rusty

Rusty is an opinionated Rust source toolchain for project-local linting and formatting rules that sit alongside `rustfmt`, Clippy, and `cargo test`.

It is built for rules that are specific to a repository's safety, readability, and source organization policy:

- stable kebab-case rule ids;
- syntax-aware diagnostics with source locations;
- auditable per-rule override comments;
- conservative formatter edits for project style.

## Usage

Run lint checks on the current directory:

```sh
cargo run --bin rusty -- check .
```

Format Rust files with Rusty formatter rules:

```sh
cargo run --bin rusty -- format .
```

Check formatting without writing changes:

```sh
cargo run --bin rusty -- format --check .
```

When no command is provided, Rusty checks the current directory:

```sh
cargo run --bin rusty
```

Run through the flake app:

```sh
nix run github:yuri-xyz/rusty -- check .
```

## Lint Rules

| Rule | Policy |
| --- | --- |
| `no-block-comments` | Disallows block comments. Use `//`, `///`, or `//!` comments instead. |
| `no-unsafe` | Disallows `unsafe` blocks unless they have a Rusty override with a justification. |
| `no-unwrap` | Disallows `.unwrap()`. Use `.expect("...")` with a concrete reason instead. |
| `no-todo-comments` | Disallows `TODO`, `FIXME`, and `XXX` comments. Track work outside source files. |
| `no-inline-tests` | Requires `#[test]` functions to live under a `tests/` directory. |
| `no-inline-modules` | Disallows inline `mod name { ... }` bodies. Move module bodies into files. |
| `max-function-args` | Limits functions to four explicit parameters. Method receivers do not count. |
| `max-function-lines` | Limits function bodies to 80 code lines. Blank and comment-only lines do not count. |
| `max-impl-lines` | Limits `impl` blocks to 80 code lines. Blank and comment-only lines do not count. |
| `max-nesting-depth` | Limits nested control flow to four levels. |
| `max-struct-fields` | Limits structs to 12 fields. |
| `max-file-lines` | Limits Rust source files to 700 code lines. Blank and comment-only lines do not count. |

Current lint diagnostics are reported as errors.

## Formatter Rules

| Rule | Policy |
| --- | --- |
| `block-spacing` | Groups direct child constructs inside blocks with blank lines between different kinds of work. Multi-line constructs are isolated from adjacent constructs. |

The formatter only rewrites whitespace-only gaps between adjacent constructs, so comments are not moved or detached from nearby code.

## Overrides

Rusty supports source-level overrides with comments:

```rust
// rusty::override(<rule-id>): <explanation>
```

Example:

```rust
// rusty::override(no-unsafe): required for FFI call; pointer validity is checked by the caller.
unsafe {
  ffi_call(ptr);
}
```

Overrides are opt-in per rule. A rule can only be overridden when its implementation explicitly allows it, and the override must include a non-empty explanation.

Currently overrideable rules:

| Rule | Reason |
| --- | --- |
| `no-unsafe` | Unsafe code can be necessary, but each use needs a local soundness explanation. |
| `max-struct-fields` | Some data models are intentionally wide and should document why splitting would be worse. |

## Workspace

This repository is a Cargo workspace:

```text
crates/
  rusty-core/  Rule definitions, parser integration, diagnostics, and text edits.
  rusty-cli/   Command-line interface for checking and formatting Rust source files.
```

Repository formatting is configured in `rustfmt.toml` with two-space indentation:

```toml
tab_spaces = 2
```

## Development

Enter the Nix development shell:

```sh
nix --extra-experimental-features 'nix-command flakes' develop
```

Or enable direnv:

```sh
direnv allow
```

Run the standard checks:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo run --bin rusty -- check .
cargo run --bin rusty -- format --check .
```

## NixOS Installation

Add Rusty as a flake input and include the package:

```nix
{
  inputs.rusty.url = "github:yuri-xyz/rusty";

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
