# this ensures that just running just give a list of commands
[private]
list:
    just --list

choose:
    just --choose

alias c := choose

export RUST_BACKTRACE := "1"

[macos]
[group('setup')]
install_cross_toolchains_on_apple_silicon:
    rustup target add x86_64-apple-darwin

    brew tap messense/macos-cross-toolchains
    
    brew install x86_64-unknown-linux-gnu
    rustup target add x86_64-unknown-linux-gnu
    
    brew install aarch64-unknown-linux-gnu
    rustup target add aarch64-unknown-linux-gnu
    
    brew install x86_64-unknown-linux-musl
    rustup target add x86_64-unknown-linux-musl

    brew install aarch64-unknown-linux-musl
    rustup target add aarch64-unknown-linux-musl

[linux]
[group('setup')]
build_linux_host_agents:
    # install cross compilation toolchains (e.g. from musl.cc)
    # running gnu linkers for musl targets generally works, and these are more widely available on distros. The reverse may also be true on musl based distros
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-musl &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target aarch64-unknown-linux-musl &
    wait

[macos]
[group('setup')]
build_all_host_agents:
    cargo build --release --bin shuthost_host_agent --target aarch64-apple-darwin &
    cargo build --release --bin shuthost_host_agent --target x86_64-apple-darwin &
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-musl &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc cargo build --release --bin shuthost_host_agent --target aarch64-unknown-linux-musl &
    wait

[group('devops')]
deploy_branch_on_metal:
    unset DATABASE_URL && cargo build --release --bin shuthost_coordinator --features include_linux_agents,include_macos_agents && sudo ./target/release/shuthost_coordinator install --port 8081

[group('projectmanagement')]
[working-directory("frontend")]
build_graphs:
    npm run build:diagrams

[group('devops')]
[confirm]
clean:
    cargo clean && cargo fetch
    cd frontend && rm -rf node_modules && npm ci

[group('projectmanagement')]
update_dependencies:
    cargo update --verbose
    cd frontend && npm update

alias deps := update_dependencies

[group('devops')]
build_gh_pages +flags="":
    ./scripts/build-gh-pages.sh {{flags}}

export DATABASE_URL := "sqlite:" + justfile_directory() + "/shuthost.db"
export SQLX_OFFLINE := "true"

[group('database')]
db_create:
    cargo sqlx database drop
    cargo sqlx database create
    cargo sqlx migrate run --source coordinator/migrations

[group('database')]
[group('projectmanagement')]
[working-directory("coordinator")]
db_update_sqlx_cache:
    cargo sqlx prepare

[group('database')]
[group('projectmanagement')]
[working-directory("coordinator")]
db_add_migration name:
    sqlx migrate add {{name}}

[group('tests')]
test_all:
    cargo test --workspace

[script]
[group('tests')]
[group('projectmanagement')]
coverage:
    export COVERAGE=1
    export SKIP_BUILD=1
    eval "$(cargo llvm-cov show-env --export-prefix --remap-path-prefix)"
    # note: removes binaries too (only in combination with previous line)
    cargo llvm-cov clean --workspace
    # note: building for a provided target instead of native
    # doesnt seem to result in an instrumented binary
    . ./scripts/helpers.sh && build_gnu
    cd frontend && npm run test && cd ..
    just build_gh_pages --provided-binary=target/debug/shuthost_coordinator
    cargo test --workspace --all-targets
    # ought to run this before the musl tests to ensure its running the gnu binary (not that it should make a huge difference)
    ./scripts/tests/direct-control-ubuntu.sh ./target/debug/shuthost_host_agent
    ./scripts/tests/service-installation-systemd.sh ./target/debug/shuthost_coordinator
    ./scripts/snapshot_files/systemd.sh ./target/debug/shuthost_coordinator
    # now run musl tests
    . ./scripts/helpers.sh && build_musl
    ./scripts/tests/direct-control-alpine.sh ./target/debug/shuthost_host_agent
    ./scripts/tests/direct-control-pwsh.sh ./target/debug/shuthost_host_agent
    ./scripts/tests/service-installation-openrc.sh ./target/debug/shuthost_coordinator
    ./scripts/snapshot_files/openrc.sh ./target/debug/shuthost_coordinator
    ./scripts/snapshot_files/compose_and_self_extracting.sh ./target/debug/shuthost_coordinator
    cargo llvm-cov report --lcov --output-path lcov.info --ignore-filename-regex ".*cargo/registry/src/.*|tests/rs_integration/.*"
    cargo llvm-cov report --html --output-dir target/coverage --ignore-filename-regex ".*cargo/registry/src/.*|tests/rs_integration/.*"

[group('tests')]
cargo_deny:
    cargo --locked deny --all-features check --hide-inclusion-graph

alias deny := cargo_deny

[group('projectmanagement')]
[working-directory("docs/examples")]
update_test_config_diffs:
    diff -u example_config.toml example_config_with_client_and_host.toml > example_config_with_client_and_host.toml.patch || true
    diff -u example_config.toml example_config_oidc.toml > example_config_oidc.toml.patch || true
    diff -u example_config.toml example_config_external.toml > example_config_external.toml.patch || true

[group('setup')]
[working-directory("docs/examples")]
patch_test_configs:
    patch example_config.toml -o example_config_with_client_and_host.toml < example_config_with_client_and_host.toml.patch 
    patch example_config.toml -o example_config_oidc.toml < example_config_oidc.toml.patch
    patch example_config.toml -o example_config_external.toml < example_config_external.toml.patch

[script]
[group('tests')]
update_file_snapshots:
    . ./scripts/helpers.sh && build_gnu
    ./scripts/snapshot_files/systemd.sh ./target/debug/shuthost_coordinator
    . ./scripts/helpers.sh && build_musl
    ./scripts/snapshot_files/openrc.sh ./target/debug/shuthost_coordinator
    ./scripts/snapshot_files/compose_and_self_extracting.sh ./target/debug/shuthost_coordinator

[group('tests')]
install_test_scripts:
    ./scripts/tests/enduser_install_scripts.sh
    ./scripts/tests/direct-control-alpine.sh
    ./scripts/tests/direct-control-pwsh.sh
    ./scripts/tests/direct-control-ubuntu.sh
    ./scripts/tests/service-installation-openrc.sh
    ./scripts/tests/service-installation-systemd.sh

[group('tests')]
typos:
    typos

alias typo := typos

[group('tests')]
cargo_clippy +flags="":
    cargo clippy --workspace --all-targets {{flags}}

alias clippy := cargo_clippy

[group('projectmanagement')]
rustfix_yolo:
    __CARGO_FIX_YOLO=1 cargo clippy --fix --workspace --all-targets --allow-dirty
    cargo fmt

alias yolo := rustfix_yolo

[group('tests')]
playwright +flags="":
    cd frontend && npm ci && npx tsc --noEmit && npx playwright test {{flags}}

[group('tests')]
pixelpeep:
    PIXELPEEP=1 just playwright

[group('tests')]
playwright_report:
    cd frontend && npx playwright show-report

[group('tests')]
cargo_tests +flags="":
    cargo test --workspace {{flags}}

alias ct := cargo_tests
alias ctests := cargo_tests

[group('projectmanagement')]
[script('bash')]
release TYPE skip_coverage_and_file_snapshots="false":
    set -euo pipefail
    git fetch
    CURRENT_BRANCH=$(git branch --show-current)
    if [ "$CURRENT_BRANCH" != "main" ]; then
        echo "Not on main branch. Aborting."
        exit 1
    fi
    if [ "$(git rev-list --count HEAD..origin/main)" -gt 0 ]; then
        echo "Remote main has commits that are not in local main. Please pull first. Aborting."
        exit 1
    fi
    echo "Starting {{TYPE}} release process with skip_coverage_and_file_snapshots={{skip_coverage_and_file_snapshots}}..."
    just update_dependencies
    cargo fmt
    just update_test_config_diffs
    just patch_test_configs
    just cargo_deny
    just db_update_sqlx_cache
    if [[ "{{skip_coverage_and_file_snapshots}}" != "true" ]]; then
        just update_file_snapshots
        just coverage
    else
        echo "Skipping coverage and file snapshots..."
    fi
    CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
    IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
    case {{TYPE}} in
        patch)
            NEW_PATCH=$((PATCH + 1))
            NEW_VERSION="$MAJOR.$MINOR.$NEW_PATCH"
            ;;
        minor)
            NEW_MINOR=$((MINOR + 1))
            NEW_VERSION="$MAJOR.$NEW_MINOR.0"
            ;;
        major)
            NEW_MAJOR=$((MAJOR + 1))
            NEW_VERSION="$NEW_MAJOR.0.0"
            ;;
        *)
            echo "Invalid type: {{TYPE}}. Use patch, minor, or major."
            exit 1
            ;;
    esac
    echo "Bumping version from $CURRENT_VERSION to $NEW_VERSION"
    sed -i "s/version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
    cd frontend
    npx playwright test --update-snapshots all
    cd ..
    git add .
    while true; do
        read -p "Please check the new snapshots and any other changes. Continue with commit? (y/N/s for shell) " -n 1 -r REPLY
        echo
        case $REPLY in
            [Yy])
                break
                ;;
            [Nn])
                echo "Release aborted."
                exit 1
                ;;
            [Ss])
                echo "Dropping into subshell ($SHELL). Type 'exit' to return."
                $SHELL || true
                ;;
            *)
                echo "Invalid input. Please enter y, n, or s."
                ;;
        esac
    done

    echo "Enter changes not tracked in a PR for this release (end with Ctrl+D for none):"
    MANUAL_CHANGES=$(cat)

    if [ -z "$MANUAL_CHANGES" ]; then
        MANUAL_CHANGES="(None)"
    fi

    git commit -m "Create Release $NEW_VERSION" -m "Automated release tasks performed:" -m "* Updated dependencies" -m "* Formatted code with cargo fmt" -m "* Bumped version to $NEW_VERSION" -m "* Updated Playwright snapshots" -m "* Updated config patches" -m "* Updated file snapshots" -m "* Updated sqlx cache (if required)" -m "* Updated coverage report" -m "" -m "## Changes not tracked in a PR:" -m "$MANUAL_CHANGES"
    git tag "$NEW_VERSION"
    # we need separate pushes here to ensure that the tag push triggers the *release-tag jobs
    git push origin refs/heads/main
    git push origin "$NEW_VERSION"
    echo "{{TYPE}} release $NEW_VERSION completed successfully!"
