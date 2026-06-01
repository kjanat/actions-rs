# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-06-01

### Added

- The `format!`-style logging macros (`debug!`, `info!`, `notice!`, `warning!`,
  `error!`, `group!`) are now re-exported from the `log` module and the prelude,
  so they can be called as `actions_rs::log::group!` and appear in the `log`
  module documentation alongside the functions they wrap. The existing
  crate-root paths (`actions_rs::group!`, …) are unchanged.

[Unreleased]: https://github.com/kjanat/actions-rs/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/kjanat/actions-rs/compare/v0.1.0...v0.1.1
