# Contributing to shuthost

Thank you for your interest in contributing to shuthost! We welcome contributions to improve the project across all supported platforms.

## Minimum Supported Rust Version (MSRV)
- The MSRV is generally set to the latest stable `rustc` as of the most recent commit.

## Frontend Dependencies
- The project depends on an installed `npm` to build frontend assets ([TailwindCSS and TypeScript](../coordinator/assets/package.json)).
- `npm` is invoked from [`coordinator/build.rs`](../coordinator/build.rs) to avoid missing errors from missed manual invocations of `tsc` or `tailwindcss`. This means `npm` is a hard dependency for building the project.

## Playwright frontend tests

The repository includes ARIA snapshot and visual regression tests based on Playwright under `frontend-tests/`. They run a local instance of the Rust backend and use Playwright's Chromium to exercise the Web UI and collect snapshots.

Quick checklist:
- Install Node dependencies and Playwright (from the repository root):
  - `cd frontend-tests`
  - `npm ci`
  - `npm run install-chromium`
 - Install Git LFS and fetch visual-regression assets (if you only run ARIA snapshot tests, this is not needed):
   - Install Git LFS for your OS (see https://git-lfs.com/)
   - `git lfs install`
   - `git lfs pull`

Running tests:
- From `frontend-tests/` run:
  - `npm test`

Notes and tips:
- Tests run fully parallel by default. Each worker uses a per-worker port computed as `8081 + workerIndex` via the environment variables `TEST_PARALLEL_INDEX` or `TEST_WORKER_INDEX`. You can force a single-worker run by exporting `TEST_WORKER_INDEX=0` before starting tests.
- To install missing system dependencies for Chromium on Linux (Debian/Ubuntu) run:
  - `npx playwright install-deps`
- Playwright collects traces and generates an HTML report by default; run `npx playwright show-report` after a run for debugging.

## Host Agent Artifacts
- Host agent binaries (the binaries that are run on every host to be controlled) and other artifacts are included in the build using `include_bytes!` (for portability of the controller binary), so they must be present in the expected locations (e.g. the Cargo target directory for host agents).
- Building macOS agents on Linux is not supported. To avoid build failures:
  - You can comment out the default features `include_macos_agents` and `include_linux_agents` in your local [`coordinator/Cargo.toml`](../coordinator/Cargo.toml). Do not commit these changes ;-P.
  - Alternatively, for supported agents, use cross-compilation toolchains as described in the [`Justfile`](../Justfile) - similar to Gnu Make - to build the required agents in release mode.

## Shell Scripts & Portability
- To support many platforms, shell scripts should **not** use bashisms.
- Scripts should be POSIX-compliant and tested with `sh`.
- [`shellcheck`](https://www.shellcheck.net/) should suffice to catch bashisms, especially since scripts have a shebang. Please lint your scripts before submitting.
- There is also a ShellCheck VSCode extension available for convenient linting in your editor.
- The [GitHub pipeline](../.github/workflows/main.yaml) should catch remaining bashisms and portability issues.

## Rust Lints
- The project enables a variety of pedantic and strict Rust lints to maintain code quality.
- Lints may be ignored using `#[expect(clippy::lint, reason="provide a reason")]` if necessary.
- Please try to address all lint warnings and errors before submitting a pull request (if it's difficult its okay to ask for help in a PR).
- Run `cargo clippy --workspace` to check for lint issues (including warnings, which will fail the pipeline).

## Additional Quality Checks
- This project uses [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) for dependency and license checks.
- It also uses [`typos-cli`](https://docs.rs/crate/typos-cli/latest) to catch spelling mistakes in code and documentation.
- Applying a `cargo fmt` before submitting a PR is appreciated.

## CI Pipeline Notes
- Occasionally, the pipeline may fail in Ubuntu container-based workflows when installing dependencies. If this happens, try re-running the affected job. The cause is unclear.

## How to Contribute
1. Fork the repository and create your branch from `main`.
2. Make your changes, ensuring all tests and lints pass.
3. Submit a pull request with a clear description of your changes.

## Code of Conduct
Please be respectful and constructive in all interactions.

---
If you have any questions, feel free to open an issue.
