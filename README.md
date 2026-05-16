# actions-rs

<!-- rumdl-disable MD013 -->

A **zero-dependency**, `#![forbid(unsafe_code)]` Rust toolkit for talking to
the GitHub Actions runner from a binary/Docker action or any CI step.\
It speaks the *workflow-command* and *environment-file* protocols — an
independent, unofficial Rust port of [`@actions/core`] (faithful API and
semantics, with [deliberate safety-first departures](#differences-from-actionscore)).\
Not affiliated with or endorsed by GitHub or the `@actions/toolkit` project.

## What it's for (and the one thing it does best)

The flagship is a precise **annotation builder**: `notice`/`warning`/`error`
with `file` + line/column **ranges** and a title, using the exact
data-vs-property percent-encoding `@actions/core` uses (`%`, `\r`, `\n`
everywhere; `:`, `,` additionally in properties). Getting that encoding wrong
silently corrupts annotations in the GitHub UI — this crate gets it right and
unit-tests the encoding tables directly.

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
  std-only** random heredoc delimiter (the [CVE-2020-15228] injection class) and
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
  environment-file values (the [CVE-2020-15228] class of bug), strict YAML 1.2
  boolean inputs.
- **Honest errors.** Filesystem/parse operations return `Result`; pure stdout
  commands are infallible — no fake error channel.
- **Modern + compatible.** Uses `GITHUB_ENV`/`GITHUB_OUTPUT`/… directly; only
  `set_output` / `save_state` keep deprecated stdout fallbacks where GitHub
  still supports them.

## Differences from `@actions/core`

Faithful to the protocol, but **deliberately divergent** where Node semantics
would be unsafe or dishonest in Rust. None of these change the bytes the runner
sees; they change what the *calling process* can rely on:

1. **No `std::env` mutation.** `@actions/core`'s `exportVariable` / `addPath`
   also write `process.env` so the *current* process sees the change. That
   write is [`unsafe` in Rust edition 2024][rust-2024-set-var] (it is unsound
   when another thread may be running) and this crate is
   `#![forbid(unsafe_code)]`, so `export_var` / `add_path` only write the env
   file. For same-process visibility use the safe overlay: `overlay_var`,
   `overlay_path`, or `apply_overlay(&mut Command)` for child processes.
2. **No retired-command fallback for env/PATH.** `set_output` / `save_state`
   keep the merely-*deprecated* `::set-output::` / `::save-state::` stdout
   fallback. `export_var` / `add_path` do **not** fall back to
   `::set-env::` / `::add-path::` — GitHub *disabled* those ([CVE-2020-15228],
   [changelog][gha-setenv]) — so they return `Error::UnavailableFileCommand`
   off-runner instead of emitting a command the runner ignores.
3. **Injection guard.** A `\r`/`\n` in an output/state/env key or a `PATH`
   entry returns `Error::InvalidName` rather than silently injecting extra
   env-file entries (the [CVE-2020-15228] class). The heredoc delimiter is
   collision-checked → `Error::DelimiterCollision`.
4. **Honest errors.** Filesystem/parse operations return `Result`; pure
   stdout commands stay infallible. No swallowed errors, no fake channel.
5. **Summary escaped by default.** `Summary` HTML-escapes text and attributes;
   raw HTML is explicit opt-in via `SummaryText::html`. `@actions/core`
   concatenates raw HTML.
6. **Deferred failure, not `process.exit`.** `set_failed` sets a flag instead
   of exiting; `is_failed()` / `exit_code() -> std::process::ExitCode` let
   `main` decide *when* to exit, so destructors and cleanup still run.

## Honest comparison with the alternatives

This is not the only crate in this space. Pick the right tool:

| Crate                 | Deps                         | Annotation builder (file+range) | Env files | Typed inputs    | Summary | API client / derive                  | Notes                                 |
| --------------------- | ---------------------------- | ------------------------------- | --------- | --------------- | ------- | ------------------------------------ | ------------------------------------- |
| **actions-rs** (this) | **zero**, `forbid(unsafe)`   | **yes, exact encoding, range**  | yes       | yes (`FromStr`) | yes     | no                                   | annotation-first; smallest, strictest |
| [`gha`]               | zero                         | not as a builder                | yes       | basic           | yes     | no                                   | closest overlap; mature, macro-style  |
| [`ghactions`]         | octocrab/serde/derive (opt.) | no                              | yes       | yes (derive)    | no      | yes (`#[derive(Actions)]`, octocrab) | biggest, batteries-included           |
| [`github-actions`]    | `uuid` (opt.)                | no                              | yes       | yes             | yes     | no                                   | similar, tiny                         |
| [`actions-core`]      | `uuid 0.8`                   | no (loose `file`/`line`/`col`)  | no[^ac]   | basic           | no      | no                                   | v0.0.2 (2020-04-01)                   |

[^ac]: From its published `core.rs` (latest release 0.0.2, 2020-04-01, per [crates.io][actions-core-cratesio]):
    `set_env` / `add_path` emit the stdout `::set-env::` / `::add-path::` workflow commands.\
    GitHub announced the disabling of that command pair in October 2020 ([changelog][gha-setenv], [CVE-2020-15228])
    — distinct from the `::set-output::` / `::save-state::` pair, which was later only deprecated.

**When to use this one:** you want zero dependencies and `#![forbid(unsafe_code)]`
in a CI/security context, and you care about correct, ranged annotations.\
**When not to:** you want a GitHub API client, `action.yml` generation, or
derive-driven input structs — use [`ghactions`].\
If you just want zero-dep logging/env helpers and don't need the annotation
builder, [`gha`] is mature and fine.

**Not provided here:** REST/GraphQL client, OIDC, `action.yml`
derive/codegen, tool cache, glob, command exec. These are deliberately a
different crate's job — pair this with one that covers them.

## Quick start

The asserts below are real: this block is compiled **and run** as a doctest
(see `tests/` and the `cfg(doctest)` README harness), so the documented
behaviour is verified on every `cargo test`.

```rust
use actions_rs::{Annotation, AnnotationSpan, Cell, Summary, SummaryText, log, output};

fn main() -> actions_rs::Result<()> {
    // Typed, validated input. `INPUT_VERBOSE` is unset here, so the default
    // is used. (`bool_input` is strict YAML 1.2: only true/True/TRUE/...)
    let verbose = actions_rs::input::bool_input("verbose").unwrap_or(false);
    assert!(!verbose);

    // Register a secret; the runner redacts it from all later log output.
    log::set_secret("hunter2");

    // Annotation with a precise source span -> one workflow command on stdout:
    // ::warning title=lint,file=src/main.rs,line=42,col=5,endColumn=9::unused import
    Annotation::new()
        .file("src/main.rs")
        .span(AnnotationSpan::Column {
            line: 42,
            start: 5,
            end: Some(9),
        })
        .title("lint")
        .warning("unused import");

    // `format!`-style macro, exported at the crate root.
    actions_rs::warning!("verbose = {verbose}");

    // Panic-safe group: emits ::group::/::endgroup:: even if the body
    // panics, and forwards the body's value.
    let answer = actions_rs::group!("build", { 6 * 7 });
    assert_eq!(answer, 42);

    // Build a job summary. Text is HTML-escaped by default; raw HTML is an
    // explicit opt-in via `SummaryText::html`. `stringify` renders it in
    // memory (no runner needed), so the structure is verifiable right here.
    let mut summary = Summary::new();
    summary
        .heading("Result", 2)
        .details(
            "note: <b> stays literal",
            SummaryText::html("<strong>raw opt-in</strong>"),
        )
        .table([vec![Cell::header("answer"), Cell::new(answer.to_string())]]);
    let rendered = summary.stringify();
    assert!(rendered.contains("<h2>Result</h2>"));
    assert!(rendered.contains("note: &lt;b&gt; stays literal")); // escaped by default
    assert!(rendered.contains("<strong>raw opt-in</strong>")); // explicit html(): not escaped
    assert!(rendered.contains(r#"<td colspan="1" rowspan="1">42</td>"#));

    // Runner-only side effects. `GITHUB_OUTPUT`/`GITHUB_ENV` only exist on a
    // runner, and `export_var`/`add_path` have NO stdout fallback (GitHub
    // disabled `::set-env::`/`::add-path::`), so they error off-runner by
    // design -- gate them behind the runtime check.
    if actions_rs::env::is_github_actions() {
        log::info("running inside GitHub Actions");
        output::set_output("answer", answer)?;

        // `export_var` writes GITHUB_ENV for *later* steps. It does not (and
        // cannot safely) mutate this process's env -- `std::env::set_var` is
        // `unsafe` in edition 2024. To observe it same-process, read the safe
        // overlay instead of `std::env::var`.
        output::export_var("BUILD_OK", true)?;
        assert_eq!(output::overlay_var("BUILD_OK").as_deref(), Some("true"));

        summary.write()?;
    }
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

See [`examples/demo.rs`] for a runnable tour.

## Compatibility

- Rust 1.95+ (edition 2024), `#![forbid(unsafe_code)]`, zero dependencies.
- Dual-licensed **MIT OR Apache-2.0**.
- Unrelated to the archived `actions-rs` GitHub org (`setup-rust` actions);
  this is an independent crate of the same (previously unregistered) name.

[CVE-2020-15228]: https://nvd.nist.gov/vuln/detail/cve-2020-15228
[rust-2024-set-var]: https://doc.rust-lang.org/edition-guide/rust-2024/newly-unsafe-functions.html#stdenvset_var-remove_var
[gha-setenv]: https://github.blog/changelog/2020-10-01-github-actions-deprecating-set-env-and-add-path-commands/
[actions-core-cratesio]: https://crates.io/crates/actions-core/versions
[`@actions/core`]: https://github.com/actions/toolkit/tree/main/packages/core
[`actions-core`]: https://crates.io/crates/actions-core
[`examples/demo.rs`]: ./examples/demo.rs
[`gha`]: https://crates.io/crates/gha
[`ghactions`]: https://crates.io/crates/ghactions
[`github-actions`]: https://crates.io/crates/github-actions
