# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Documentation set v0.4: SRS, PRD, high-level design, detailed design,
  architecture & maintenance guide, coding standards (docs/01–06).
- Repository skeleton per target layout (engine/, control/, proto/, tests/,
  benchmarks/, deploy/, scripts/, ui/).
- Minimal entry points: `flux-worker` (Rust workspace binary), `flux-master`
  and `flux` CLI (Go module `github.com/flux-labs/flux`, stdlib-only CLI).
