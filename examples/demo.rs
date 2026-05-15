//! Run with `cargo run --example demo` and eyeball the workflow commands.
//!
//! To see the env-file/summary side-effects, point the variables at temp
//! files first, e.g.:
//!
//! ```sh
//! GITHUB_OUTPUT=$(mktemp) GITHUB_STEP_SUMMARY=$(mktemp) \
//!     cargo run --example demo
//! ```

use actions_rs::{Annotation, Cell, Summary, env, log, output};

fn main() {
    log::info(format!(
        "in GitHub Actions: {} | CI: {} | step-debug: {}",
        env::is_github_actions(),
        env::is_ci(),
        log::is_debug()
    ));

    let ctx = actions_rs::Context::new();
    log::info(format!(
        "repo={:?} ref={:?} sha={:?}",
        ctx.repository(),
        ctx.ref_name(),
        ctx.sha()
    ));

    // Located annotation with a line range — should print:
    // ::warning title=demo,file=src/lib.rs,line=10,endLine=12::heads up
    Annotation::new()
        .file("src/lib.rs")
        .line(10)
        .end_line(12)
        .title("demo")
        .warning("heads up: this span looks suspicious");

    // Escaping check: newline in data, colon/comma in a property.
    Annotation::new()
        .title("type: mismatch, really")
        .error("line one\nline two");

    let total = log::group("expensive step", || {
        log::info("...working...");
        2 + 2
    });
    log::info(format!("group returned {total}"));

    actions_rs::warning!("formatted macro: {} items left", 7);

    output::set_output("answer", 42).expect("set_output");
    output::export_var("DEMO_FLAG", true).expect("export_var");

    let mut summary = Summary::new();
    summary
        .heading("Demo Report", 2)
        .raw("Built by the `demo` example.", true)
        .table([
            vec![Cell::header("Check"), Cell::header("Result")],
            vec![Cell::new("clippy"), Cell::new("pass")],
            vec![Cell::new("tests"), Cell::new("36 pass")],
        ])
        .code_block("cargo test", Some("sh"));
    summary.write().expect("write summary");
    log::info("summary written (if GITHUB_STEP_SUMMARY was set)");
}
