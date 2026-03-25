# Requirements Document

## Introduction

This feature adds lint and formatting enforcement to the CI pipeline for the Atomic IP Marketplace Rust workspace. Currently, no `cargo clippy` or `cargo fmt` checks run in CI, allowing code style inconsistencies and clippy-detectable bugs to accumulate silently. The goal is to add dedicated CI jobs for clippy and rustfmt, fix all existing warnings, and enforce a zero-warning policy on every pull request.

## Glossary

- **CI_Pipeline**: The GitHub Actions workflow defined in `.github/workflows/ci.yml`
- **Clippy_Job**: The CI job that runs `cargo clippy` across the workspace
- **Fmt_Job**: The CI job that runs `cargo fmt --check` across the workspace
- **Workspace**: The Cargo workspace containing `contracts/atomic_swap`, `contracts/ip_registry`, and `contracts/zk_verifier`
- **PR**: A GitHub pull request targeting any branch in the repository

## Requirements

### Requirement 1: Clippy Lint Enforcement in CI

**User Story:** As a maintainer, I want clippy to run on every push and PR, so that avoidable bugs and code quality issues are caught automatically before merging.

#### Acceptance Criteria

1. WHEN the CI_Pipeline is triggered by a push or PR, THE Clippy_Job SHALL run `cargo clippy --workspace -- -D warnings` against all crates in the Workspace.
2. WHEN `cargo clippy` produces one or more warnings, THE Clippy_Job SHALL exit with a non-zero status code, causing the CI_Pipeline to fail.
3. WHEN `cargo clippy` produces zero warnings, THE Clippy_Job SHALL exit with status code 0, allowing the CI_Pipeline to continue.
4. THE Clippy_Job SHALL install the `clippy` component via `dtolnay/rust-toolchain` before executing the lint step.
5. THE Clippy_Job SHALL reuse the Cargo registry cache using `actions/cache` keyed on `Cargo.lock` to avoid redundant downloads.

### Requirement 2: Formatting Check Enforcement in CI

**User Story:** As a maintainer, I want rustfmt to verify code formatting on every push and PR, so that style inconsistencies are caught automatically before merging.

#### Acceptance Criteria

1. WHEN the CI_Pipeline is triggered by a push or PR, THE Fmt_Job SHALL run `cargo fmt --all --check` against all crates in the Workspace.
2. WHEN `cargo fmt --check` detects formatting differences, THE Fmt_Job SHALL exit with a non-zero status code, causing the CI_Pipeline to fail.
3. WHEN `cargo fmt --check` detects no formatting differences, THE Fmt_Job SHALL exit with status code 0, allowing the CI_Pipeline to continue.
4. THE Fmt_Job SHALL install the `rustfmt` component via `dtolnay/rust-toolchain` before executing the format check step.
5. THE Fmt_Job SHALL reuse the Cargo registry cache using `actions/cache` keyed on `Cargo.lock` to avoid redundant downloads.

### Requirement 3: Zero-Warning Policy on Pull Requests

**User Story:** As a maintainer, I want all PRs to be blocked from merging when clippy or fmt checks fail, so that the zero-warning policy is enforced consistently.

#### Acceptance Criteria

1. WHEN a PR is opened or updated, THE CI_Pipeline SHALL execute both the Clippy_Job and the Fmt_Job as required status checks.
2. IF the Clippy_Job fails on a PR, THEN THE CI_Pipeline SHALL report a failed status on the PR, preventing merge until the warnings are resolved.
3. IF the Fmt_Job fails on a PR, THEN THE CI_Pipeline SHALL report a failed status on the PR, preventing merge until formatting is corrected.
4. THE Workspace SHALL contain zero clippy warnings under `cargo clippy --workspace -- -D warnings` before the feature is considered complete.
5. THE Workspace SHALL produce no formatting differences under `cargo fmt --all --check` before the feature is considered complete.

### Requirement 4: Toolchain Component Availability

**User Story:** As a developer, I want the CI environment to always have the correct Rust toolchain components installed, so that lint and format jobs never fail due to missing tools.

#### Acceptance Criteria

1. THE Clippy_Job SHALL specify `components: clippy` in the `dtolnay/rust-toolchain` action step.
2. THE Fmt_Job SHALL specify `components: rustfmt` in the `dtolnay/rust-toolchain` action step.
3. WHEN the stable Rust toolchain does not include `clippy` or `rustfmt` by default, THE CI_Pipeline SHALL install the missing component explicitly before running the corresponding job step.
