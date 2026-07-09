# Testing Guide

This project uses three test layers with different goals and runtimes.

## 1) Unit tests (`src/**`)

- Scope: pure logic, small helpers, model behavior, UI layout math.
- Speed: fastest.
- Command:

```bash
cargo test --lib
```

## 2) Integration tests (`tests/integration.rs`)

- Scope: scanner + cleaner behavior against real temporary filesystem fixtures.
- Layout:
  - `tests/common/mod.rs`: shared fixture and worker helpers
  - `tests/integration/config_cli.rs`: config parsing and CLI parsing behavior
  - `tests/integration/scanners/*`: per-category scanner behavior
  - `tests/integration/cleaning/clean_flow.rs`: clean action behavior and progress events
- Command:

```bash
cargo test --test integration
```

## 3) End-to-end tests (`tests/e2e.rs`)

- Scope: real `mac-cleaner` binary process execution (`scan`/`clean` flows).
- Layout:
  - `tests/e2e/fixtures.rs`: deterministic fixture builder and output normalization
  - `tests/e2e/flow.rs`: process-level scenarios
- Command:

```bash
cargo test --test e2e
```

## macOS-dependent ignored tests

Some tests intentionally touch real macOS Trash behavior and are marked `#[ignore]`.

```bash
cargo test --test integration -- --ignored
```

These are intentionally excluded from default CI because they depend on runner environment state (real Trash / external tooling behavior) and can be flaky.
These run in the dedicated macOS ignored-tests CI job.

## Recommended local workflow

```bash
# Fast confidence (unit + integration + e2e)
cargo test

# Include macOS-specific ignored tests before release
cargo test --test integration -- --ignored
```

## Test style conventions

- Prefer `given_when_then` test names.
- Keep each test focused on one behavior contract.
- Use deterministic fixtures (explicit file sizes/content, no wall-clock assumptions when avoidable).
- Assert exact outcomes where practical (counts, action, tier, progress tuple sequence).
