# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `flux-exec` crate: hand-written vectorized predicate kernel for the minimal
  slice `SUM(price) WHERE date <= bound` (`cmp_lt_scalar`, `sum_masked`,
  `select_sum`), with unit tests (empty/NULL/boundary/mismatch/overflow)
  and criterion benchmarks comparing a naive row loop, the kernel, and
  arrow compute kernels over 1M rows.
- Documentation set v0.4: SRS, PRD, high-level design, detailed design,
  architecture & maintenance guide, coding standards (docs/01–06).
- Repository skeleton per target layout (engine/, control/, proto/, tests/,
  benchmarks/, deploy/, scripts/, ui/).
- Minimal entry points: `flux-worker` (Rust workspace binary), `flux-master`
  and `flux` CLI (Go module `github.com/flux-labs/flux`, stdlib-only CLI).
