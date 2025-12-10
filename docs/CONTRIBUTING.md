# Contributing to shuthost

Thank you for your interest in contributing to shuthost! We welcome contributions to improve the project across all supported platforms.

## Minimum Supported Rust Version (MSRV)
- The MSRV is generally set to the latest stable `rustc` as of the most recent commit.

## Frontend Dependencies
- The project depends on an installed `npm` to build frontend assets ([TailwindCSS and TypeScript](../frontend/package.json)).
- `npm` is invoked from the workspace build script (or from `frontend`) to avoid missing errors from missed manual invocations of `tsc` or `tailwindcss`. This means `npm` is a hard dependency for building the project.


## Host Agent Artifacts
- Host agent binaries (the binaries that are run on every host to be controlled) and other artifacts are included in the build using `include_bytes!` (for portability of the controller binary), so they must be present in the expected locations (e.g. the Cargo target directory for host agents).
- By default, the include_macos_agents and include_linux_agents features are disabled to avoid build failures.
- To include agent binaries in the coordinator (required for downloading them, e.g. for manual tests), enable the features via command line: `--features include_linux_agents,include_macos_agents`.
- Building macOS agents on Linux is not supported.
- To build the supported agents use cross-compilation toolchains as described in the [`Justfile`](../Justfile) - this uses syntax similar to Gnu Make - to build the required agents in release mode.

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

## Running Tests

- All rust-based tests (unit tests, integration tests, and doctests) can be run with `cargo test --workspace`.
- To run a specific test, pass the test name as an argument, e.g., `cargo test --workspace test_m2m_lease_async_take_and_release`.

## Playwright frontend tests

The repository includes ARIA snapshot and visual regression tests based on Playwright under `frontend/tests/`. They run a local instance of the Rust backend and use Playwright's Chromium to exercise the Web UI and collect snapshots.

Quick checklist:
- Install Node dependencies and Playwright (from the repository root):
  - `cd frontend`
  - `npm ci`
  - `npm run install-chromium`
 - Install Git LFS and fetch visual-regression assets (if you only run ARIA snapshot tests or the accessibility tests this is not needed):
   - Install Git LFS for your OS (see https://git-lfs.com/)
   - `git lfs install`
   - `git lfs pull`

Running tests:
- From the `frontend/` folder run:
  - `npx playwright test`
  - or `npm run test`

### Accessibility tests & updating snapshots

- Accessibility checks are included in the Playwright test suite using `@axe-core/playwright` and are run as part of the normal test run.

-- To update snapshots (ARIA snapshots or Playwright's snapshot files) use Playwright's update flag. From `frontend/` you can:
  - Update all snapshots:
    - `npx playwright test --update-snapshots all`
  - Update snapshots for a single test file (faster and safer when only one test changed):
    - `npx playwright test tests/aria-snapshots.spec.ts -u`

- Notes when updating snapshots:
  - Review snapshot diffs carefully before committing. Accessibility snapshots and visual snapshots can change for legitimate reasons (UI refactors, updated wording) but should not be blindly accepted.
  - When updating snapshots for a change that affects many tests, consider updating and running a small subset first to validate the new baseline before updating everything.

Notes and tips:
- Tests run fully parallel by default. Each worker uses a per-worker port computed as `8081 + workerIndex` via the environment variables `TEST_PARALLEL_INDEX` or `TEST_WORKER_INDEX`. You can force a single-worker run by exporting `TEST_WORKER_INDEX=0` before starting tests.
- To install missing system dependencies for Chromium on Linux (Debian/Ubuntu) run:
  - `npx playwright install-deps`
- Playwright collects traces and generates an HTML report by default; run `npx playwright show-report` after a run for debugging.

### Fonts required for visual regression

The visual regression tests rely on consistent font rendering between CI and local runs. To reduce noise from cross-platform font differences please install the following fonts before running the visual tests locally (and note CI expects them to be present):

- Inter (UI sans-serif)
- Cascadia Code (monospace)

Installation examples:

- Debian/Ubuntu (CI runners use apt):

  ```sh
  sudo apt-get update
  sudo apt-get install -y fonts-inter fonts-cascadia-code
  ```

- macOS (Homebrew cask-fonts):

  ```sh
  brew tap homebrew/cask-fonts
  brew install --cask font-inter font-cascadia-code
  ```

## Additional Quality Checks
- This project uses [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) for dependency and license checks.
- It also uses [`typos-cli`](https://docs.rs/crate/typos-cli/latest) to catch spelling mistakes in code and documentation.
- Applying a `cargo fmt` before submitting a PR is appreciated.

### Install File Snapshots

The project uses install file snapshots to validate that the installation process creates the expected files on different init systems (systemd, openrc, etc.). These snapshots are TOML files listing the files installed by the coordinator and agent, along with their permissions and file types.

The snapshots are generated by running the coordinator and agent installation in isolated containers using Podman, then diffing the filesystem to capture only the added files. This ensures that the installation scripts work correctly and haven't introduced unexpected changes.

To update the snapshots (e.g., when installation scripts change), run the snapshot generation scripts located in the `scripts/` directory:

```sh
./scripts/snapshot-files-systemd.sh
./scripts/snapshot-files-openrc.sh
./scripts/snapshot-files-docker-compose.sh
```

These scripts will build the necessary binaries, run the installations in containers, and generate updated TOML files in the `install-file-snapshots/` subdirectories.

Alternatively, use the `update_file_snapshots` task in the Justfile, which runs all snapshot scripts automatically.

### Coverage Collection

The project uses a complex coverage collection process (defined in the [`Justfile`](../Justfile)) that combines Rust code coverage from both Rust tests (including integration tests) and frontend Playwright tests. This ensures comprehensive coverage, particularly for websocket codepaths that are exercised by the frontend tests, providing a more realistic assessment of code coverage than Rust tests alone would be able to provide.

If setting up coverage collection locally fails, that's acceptable - in the worst case, the `lcov.info` file can be obtained from CI test runs, as they upload this file as an artifact.

## CI Pipeline Notes
- Occasionally, the pipeline may fail in Ubuntu container-based workflows when installing dependencies. If this happens, try re-running the affected job. The cause is unclear.

## Database & sqlx tooling

This project optionally uses a local SQLite database for persistence in the coordinator. A few notes for contributors working on code that touches the database layer or uses `sqlx` query macros:

- The project ships migrations under [`coordinator/migrations`](../coordinator/migrations). The migrations enable WAL mode and create a small key/value table (`kv_store`) used to persist autogenerated auth values (see README).
- When running locally or in CI, some `sqlx` query macros require a query cache. That cache is shipped in the repo, but when you modify the queries you need to regenerate it. To do so you need the sqlx cli (install it with e.g. `cargo binstall sqlx-cli`).
After getting that, run (from the repository root):

```sh
export DATABASE_URL="sqlite:./shuthost.db"

sqlx database drop
sqlx database create
sqlx migrate run --source coordinator/migrations

cargo sqlx prepare --workspace
```

To add a new migration, run from the repository root:

```sh
cd coordinator && sqlx migrate add <migration_name>
```

This will create a new migration file in `coordinator/migrations` with the given name. After adding a migration, you may need to run the migrations and update the sqlx cache as described above.

Persistence notes and the coordinator's DB behavior are described in the top-level README; please read that section before developing or testing DB-related features.

## How to Contribute
1. Fork the repository and create your branch from `main`.
2. Make your changes, ensuring all tests and lints pass.
3. Submit a pull request with a clear description of your changes.

## Code of Conduct
Please be respectful and constructive in all interactions.

---
If you have any questions, feel free to open an issue.

## Windows Compatibility Notes

Due to platform differences, there are several known limitations when developing on Windows:

- **Frontend Build Issues**: Symlinks are not properly supported on Windows, which means `build.rs` does not pick up changes in frontend code automatically. Developers on Windows may need to manually trigger rebuilds or use alternative approaches for frontend development.
- **Service Installation**: Windows services work differently from Unix-like systems, and support for installing the agent or coordinator as services is not implemented. This is acceptable for the coordinator but breaks the core functionality of the agent, which relies on running as a service.
- **Coordinator File Watching**: File watching for changes in the config file does not work reliably on Windows. Despite attempts to implement it, the coordinator will not automatically pick up new hosts or clients without a manual restart.
