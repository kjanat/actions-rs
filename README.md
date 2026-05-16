# actions-rs

A **zero-dependency**, `#![forbid(unsafe_code)]` Rust toolkit for talking to
the GitHub Actions runner from a binary/Docker action or any CI step. It speaks
the *workflow-command* and *environment-file* protocols (the Rust analogue of
[`@actions/core`]).

[`@actions/core`]: https://github.com/actions/toolkit/tree/main/packages/core

## What it's for (and the one thing it does best)

The flagship is a precise **annotation builder**: `notice`/`warning`/`error`
with `file` + line/column **ranges** and a title, using the exact
data-vs-property percent-encoding `@actions/core` uses (`%`, `\r`, `\n`
everywhere; `:`, `,` additionally in properties). Getting that encoding wrong
silently corrupts annotations in the GitHub UI — this crate gets it right and
unit-tests the encoding tables directly.

<!-- rumdl-disable MD013 -->

```rust
use actions_rs::{Annotation, AnnotationSpan};

Annotation::new()
    .file("src/parser.rs")
    .span(AnnotationSpan::Column {
        line: 42,
        start: 5,
        end: Some(7),
    })
    .title("clippy::needless_clone")
    .warning("redundant clone of `cfg`");
// emits this single line to stdout:
// ::warning title=clippy%3A%3Aneedless_clone,file=src/parser.rs,line=42,col=5,endColumn=7::redundant clone of `cfg`
```

Around that it provides the rest of the toolkit surface:

- logging + panic-safe `group`, `mask`/`set_secret`, RAII `stop_commands`,
  `echo`, `set_failed`/`fail_now`;
- env files (`GITHUB_ENV`/`OUTPUT`/`STATE`/`PATH`) with a **collision-safe,
  std-only** random heredoc delimiter (the CVE-2020-15228 injection class) and
  deprecated stdout fallback only for output/state; reserved-name guard; safe
  same-process overlay helpers for env/PATH;
- typed inputs: strict YAML 1.2 `bool_input`, `multiline_input`,
  `input_as::<T: FromStr>`, `mask_input`;
- a fluent `Summary` builder (1 MiB guarded, escaped by default, raw HTML
  opt-in via `SummaryText::html`);
- runtime detection + a typed `Context`;
- crate-root `warning!` / `group!` / … `format!`-style macros.

## Why

- **Zero dependencies.** Std only. Nothing to audit, fast to build.
- **Correct by construction.** Percent-encoding for command data vs.
  properties, collision-checked random heredoc delimiters for multiline
  environment-file values (the CVE-2020-15228 class of bug), strict YAML 1.2
  boolean inputs.
- **Honest errors.** Filesystem/parse operations return `Result`; pure stdout
  commands are infallible — no fake error channel.
- **Modern + compatible.** Uses `GITHUB_ENV`/`GITHUB_OUTPUT`/… directly; only
  `set_output` / `save_state` keep deprecated stdout fallbacks where GitHub
  still supports them.

## Honest comparison with the alternatives

This is not the only crate in this space. Pick the right tool:

| Crate                 | Deps                         | Annotation builder (file+range) | Env files | Typed inputs    | Summary | API client / derive                  | Notes                                 |
| --------------------- | ---------------------------- | ------------------------------- | --------- | --------------- | ------- | ------------------------------------ | ------------------------------------- |
| **actions-rs** (this) | **zero**, `forbid(unsafe)`   | **yes, exact encoding, range**  | yes       | yes (`FromStr`) | yes     | no                                   | annotation-first; smallest, strictest |
| [`gha`]               | zero                         | not as a builder                | yes       | basic           | yes     | no                                   | closest overlap; mature, macro-style  |
| [`ghactions`]         | octocrab/serde/derive (opt.) | no                              | yes       | yes (derive)    | no      | yes (`#[derive(Actions)]`, octocrab) | biggest, batteries-included           |
| [`github-actions`]    | `uuid` (opt.)                | no                              | yes       | yes             | yes     | no                                   | similar, tiny                         |

[`gha`]: https://crates.io/crates/gha
[`ghactions`]: https://crates.io/crates/ghactions
[`github-actions`]: https://crates.io/crates/github-actions

**When to use this one:** you want zero dependencies and
`#![forbid(unsafe_code)]` in a CI/security context, and you care about correct,
ranged annotations. **When not to:** you want a GitHub API client, `action.yml`
generation, or derive-driven input structs — use [`ghactions`]. If you just
want zero-dep logging/env helpers and don't need the annotation builder,
[`gha`] is mature and fine.

Explicitly **out of scope**: REST/GraphQL client, OIDC, `action.yml`
derive/codegen, tool cache, glob, command exec.

## Quick start

```rust
use actions_rs::{Annotation, Cell, Summary, SummaryText, log, output};

fn main() -> actions_rs::Result<()> {
    if actions_rs::env::is_github_actions() {
        log::info("running inside GitHub Actions");
    }

    // Typed, validated input (env var `INPUT_VERBOSE`).
    let verbose = actions_rs::input::bool_input("verbose").unwrap_or(false);

    // Annotation with a source span -> rendered as a workflow command.
    Annotation::new()
        .file("src/main.rs")
        .line(42)
        .title("lint")
        .warning("unused import");

    // `format!`-style macros, exported at the crate root.
    actions_rs::warning!("verbose = {verbose}");

    // Group that closes even on panic; returns the closure's value.
    let n = actions_rs::group!("build", { 6 * 7 });

    // Outputs / exported env.
    output::set_output("answer", n)?;
    output::export_var("BUILD_OK", true)?;

    // Job summary.
    let mut s = Summary::new();
    s.heading("Result", 2)
        .details(
            "rendering",
            SummaryText::html("<strong>raw HTML opt-in</strong>"),
        )
        .table([vec![Cell::header("answer"), Cell::new(n.to_string())]]);
    s.write()?;
    Ok(())
}
```

## Module map

| Module       | Purpose                                                                                                              |
| ------------ | -------------------------------------------------------------------------------------------------------------------- |
| `log`        | annotations, `group`, `mask`/`set_secret`, `stop_commands`, `echo`, `set_failed`/`fail_now`, `exit_code`/`is_failed` |
| `annotation` | `Annotation` builder (`AnnotationSpan`, file + line/column range + title)                                            |
| `input`      | `input`, `input_required`, `bool_input`, `multiline_input`, `multiline_input_with`, `input_as::<T>`, `mask_input`    |
| `output`     | `set_output`, `save_state`, `get_state`, `export_var`, `add_path`, `overlay_var`, `overlay_path`, `apply_overlay`    |
| `summary`    | fluent `Summary` builder (`SummaryText::html` for raw HTML, 1 MiB guarded)                                           |
| `env`        | `is_github_actions`/`is_ci`/`is_debug`, `RunnerOs`, `RunnerArch`, `Context`, `vars` constants                        |
| `command`    | low-level `WorkflowCommand` for anything not covered above                                                           |

See [`examples/demo.rs`](./examples/demo.rs) for a runnable tour.

## Compatibility

- Rust 1.95+ (edition 2024), `#![forbid(unsafe_code)]`, zero dependencies.
- Dual-licensed **MIT OR Apache-2.0**.
- Unrelated to the archived `actions-rs` GitHub org (`setup-rust` actions);
  this is an independent crate of the same (previously unregistered) name.
