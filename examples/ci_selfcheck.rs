//! Self-verifying CI check. Run with `cargo run --example ci_selfcheck`.
//!
//! Writes via the crate's public API, then reads the runner's environment
//! files back and asserts the bytes are what we wrote. All logic is in Rust:
//! no shell, no `${{ }}` interpolation, cross-platform for free. Exits 1 on
//! the first failed check. When the `GITHUB_*` files are absent (local run)
//! the file-readback assertions are skipped, not failed.

use std::path::PathBuf;
use std::process::ExitCode;

use actions_rs::{Annotation, Summary, output};

fn read_env_file(var: &str) -> Option<String> {
    let path = PathBuf::from(std::env::var_os(var)?);
    std::fs::read_to_string(path).ok()
}

/// Returns the failure message, or `None` if the check passed/!skipped.
fn check(var: &str, needles: &[&str]) -> Option<String> {
    let Some(contents) = read_env_file(var) else {
        eprintln!("skip {var}: not set (local run)");
        return None;
    };
    for needle in needles {
        if !contents.contains(needle) {
            return Some(format!("{var} missing {needle:?}; got:\n{contents}"));
        }
    }
    eprintln!("ok   {var}: contains {needles:?}");
    None
}

fn main() -> ExitCode {
    // --- write via the public API (what a real action would do) ---
    if let Err(e) = output::set_output("answer", 42) {
        eprintln!("set_output failed: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(e) = output::export_var("DEMO_FLAG", true) {
        eprintln!("export_var failed: {e}");
        return ExitCode::FAILURE;
    }
    Annotation::new()
        .file("examples/ci_selfcheck.rs")
        .line(1)
        .title("ci")
        .notice("self-check ran");

    let mut summary = Summary::new();
    summary
        .heading("CI self-check", 2)
        .raw("round-trip verification", true);
    if let Err(e) = summary.write() {
        eprintln!("summary.write failed: {e}");
        return ExitCode::FAILURE;
    }

    // --- read the runner's files back and assert our bytes are there ---
    let failures: Vec<String> = [
        check("GITHUB_OUTPUT", &["answer<<", "\n42\n"]),
        check("GITHUB_ENV", &["DEMO_FLAG<<", "\ntrue\n"]),
        check("GITHUB_STEP_SUMMARY", &["<h2>CI self-check</h2>"]),
    ]
    .into_iter()
    .flatten()
    .collect();

    if failures.is_empty() {
        eprintln!("all round-trips OK");
        ExitCode::SUCCESS
    } else {
        for f in &failures {
            eprintln!("::error::{f}");
        }
        ExitCode::FAILURE
    }
}
