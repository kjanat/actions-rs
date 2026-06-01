# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-06-01

### Added

- The `format!`-style logging macros (`debug!`, `info!`, `notice!`, `warning!`,
  `error!`, `group!`) are now re-exported from the `log` module, so they can be
  called as `actions_rs::log::group!` and appear in the `log` module
  documentation alongside the functions they wrap. The existing crate-root
  (`actions_rs::group!`, …) and prelude paths are unchanged.

## [0.1.0] - 2026-05-18

### Added

- Initial zero-dependency GitHub Actions toolkit with workflow-command logging,
  ranged annotations, input parsing, env-file output/state/env/PATH helpers,
  runtime context, and job summary building.
- Panic-safe log groups, secret masking, stop-command guards, deferred failure
  state, and `format!`-style logging macros at the crate root and prelude.
- Safety-first protocol behavior: exact command escaping, collision-safe
  multiline env-file delimiters, reserved-name guards, strict YAML 1.2 boolean
  inputs, escaped summary text by default, and no unsafe same-process env
  mutation.

[Unreleased]: https://github.com/kjanat/actions-rs/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/kjanat/actions-rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/kjanat/actions-rs/releases/tag/v0.1.0
