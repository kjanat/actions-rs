//! Self-verifying CI demonstration. `cargo run --example ci_selfcheck`.
//!
//! Two jobs in one: (1) *exercise* most of the public surface so the real
//! GitHub run shows annotations, grouped logs and a rich job summary; (2)
//! *verify* by reading the runner's environment files back and asserting our
//! bytes are there. All logic is Rust — no shell, no `${{ }}`, identical on
//! every OS. Exits 1 on the first failed check; locally (no `GITHUB_*`) the
//! file-readback assertions are skipped, not failed.

use std::path::PathBuf;
use std::process::ExitCode;

use actions_rs::summary::Cell;
use actions_rs::{Annotation, Summary, env, log, output};

fn read_env_file(var: &str) -> Option<String> {
    let path = PathBuf::from(std::env::var_os(var)?);
    std::fs::read_to_string(path).ok()
}

/// `Ok(true)` = verified, `Ok(false)` = skipped (local), `Err` = mismatch.
fn check(var: &str, needles: &[&str]) -> Result<bool, String> {
    let Some(contents) = read_env_file(var) else {
        return Ok(false);
    };
    for needle in needles {
        if !contents.contains(needle) {
            return Err(format!("{var} missing {needle:?}; got:\n{contents}"));
        }
    }
    Ok(true)
}

fn main() -> ExitCode {
    // --- 1. exercise the surface (visible in the CI run) ---
    log::group("environment", || {
        log::info(format!("github_actions = {}", env::is_github_actions()));
        log::info(format!("ci             = {}", env::is_ci()));
        log::info(format!("runner os      = {:?}", env::RunnerOs::from_env()));
        let ctx = actions_rs::Context::new();
        log::info(format!("repository     = {:?}", ctx.repository()));
        log::info(format!("ref            = {:?}", ctx.ref_name()));
    });

    log::mask("hunter2-not-a-real-secret");

    Annotation::new()
        .file("examples/ci_selfcheck.rs")
        .line(50)
        .end_line(54)
        .col(5)
        .title("ci_selfcheck")
        .notice("this annotation is emitted by the example and should appear on the run");

    Annotation::new()
        .file("src/summary.rs")
        .line(1)
        .title("demo warning")
        .warning("non-fatal: proves ranged warning annotations render");

    if let Err(e) = output::set_output("answer", 42) {
        log::error(format!("set_output failed: {e}"));
        return ExitCode::FAILURE;
    }
    if let Err(e) = output::export_var("DEMO_FLAG", true) {
        log::error(format!("export_var failed: {e}"));
        return ExitCode::FAILURE;
    }

    // --- 2. verify the round-trips ---
    let results = [
        (
            "GITHUB_OUTPUT",
            check("GITHUB_OUTPUT", &["answer<<", "\n42\n"]),
        ),
        (
            "GITHUB_ENV",
            check("GITHUB_ENV", &["DEMO_FLAG<<", "\ntrue\n"]),
        ),
    ];

    // --- 3. build a real job summary from the results ---
    let mut summary = Summary::new();
    summary
        .heading("actions-rs CI self-check", 2)
        .raw(
            "Round-trip of the runner's environment-file protocol, ",
            false,
        )
        .raw("verified from Rust with zero shell.", true);
    let mut rows = vec![vec![Cell::header("check"), Cell::header("status")]];
    for (name, r) in &results {
        let status = match r {
            Ok(true) => "✅ verified",
            Ok(false) => "➖ skipped (local)",
            Err(_) => "❌ FAILED",
        };
        rows.push(vec![Cell::new(*name), Cell::new(status)]);
    }
    summary
        .table(rows)
        .code_block("cargo run --example ci_selfcheck", Some("sh"))
        .details(
            "what this proves",
            "set_output/export_var/Summary speak the runner protocol correctly.",
        );
    if let Err(e) = summary.write() {
        log::error(format!("summary.write failed: {e}"));
        return ExitCode::FAILURE;
    }
    let summary_ok = check(
        "GITHUB_STEP_SUMMARY",
        &["<h2>actions-rs CI self-check</h2>"],
    );

    // --- 4. report every check explicitly (never just "OK") ---
    let mut failed = false;
    let mut verified = 0u32;
    let mut skipped = 0u32;
    for (name, r) in results
        .into_iter()
        .chain([("GITHUB_STEP_SUMMARY", summary_ok)])
    {
        match r {
            Ok(true) => {
                verified += 1;
                log::info(format!(
                    "VERIFIED  {name}: our bytes are present in the runner file"
                ));
            }
            Ok(false) => {
                skipped += 1;
                log::info(format!("SKIPPED   {name}: variable not set (local run)"));
            }
            Err(msg) => {
                failed = true;
                log::error(format!("FAILED    {name}: {msg}"));
            }
        }
    }

    log::info(format!(
        "summary: {verified} verified, {skipped} skipped, {} failed",
        u32::from(failed)
    ));

    if failed {
        ExitCode::FAILURE
    } else if verified == 0 {
        log::warning("local run: nothing verified (GITHUB_* unset) — ran demo only");
        ExitCode::SUCCESS
    } else {
        log::info("all runner round-trips verified");
        ExitCode::SUCCESS
    }
}
