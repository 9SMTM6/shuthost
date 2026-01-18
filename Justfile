# this ensures that just running just give a list of commands
_list:
    just --list

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

build_linux_host_agents:
    # install cross compilation toolchains (e.g. from musl.cc)
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-gnu &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target aarch64-unknown-linux-gnu &
    # running gnu linkers for musl targets generally works, and these are more widely available on distros. The reverse may also be true on musl based distros
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-musl &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target aarch64-unknown-linux-musl &
    wait

build_all_host_agents_on_mac:
    cargo build --release --bin shuthost_host_agent --target aarch64-apple-darwin &
    cargo build --release --bin shuthost_host_agent --target x86_64-apple-darwin &
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-gnu &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target aarch64-unknown-linux-gnu &
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-musl &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc cargo build --release --bin shuthost_host_agent --target aarch64-unknown-linux-musl &
    wait

deploy_branch_on_metal:
    unset DATABASE_URL && cargo build --release --bin shuthost_coordinator --features include_linux_agents,include_macos_agents && sudo ./target/release/shuthost_coordinator install --port 8081

build_graphs:
    dot frontend/assets/architecture.dot -Tsvg -ofrontend/assets/generated/architecture.svg
    dot frontend/assets/architecture_simplified.dot -Tsvg -ofrontend/assets/generated/architecture_simplified.svg

clean:
    cargo clean && cargo fetch
    cd frontend && rm -rf node_modules && npm ci

update_dependencies:
    cargo update --verbose
    cd frontend && npm update

build_gh_pages +flags="":
    ./scripts/build-gh-pages.sh {{flags}}

export DATABASE_URL := "sqlite:./shuthost.db"
export SQLX_OFFLINE := "true"

db_create:
    cargo sqlx database drop
    cargo sqlx database create
    cargo sqlx migrate run --source coordinator/migrations

db_update_sqlx_cache:
    cd coordinator && cargo sqlx prepare

db_add_migration name:
    cd coordinator && sqlx migrate add {{name}}

test_all:
    cargo test --workspace

coverage:
    #!/usr/bin/env sh
    set -e
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

ci_cargo_deny:
    cargo +stable --locked deny check --hide-inclusion-graph

update_test_config_diffs:
    #!/usr/bin/env sh
    set -e
    cd docs/examples/
    diff -u example_config.toml example_config_with_client_and_host.toml > example_config_with_client_and_host.toml.patch || true
    diff -u example_config.toml example_config_oidc.toml > example_config_oidc.toml.patch || true
    diff -u example_config.toml example_config_external.toml > example_config_external.toml.patch || true

patch_test_configs:
    #!/usr/bin/env sh
    set -e
    cd docs/examples/
    patch example_config.toml -o example_config_with_client_and_host.toml < example_config_with_client_and_host.toml.patch 
    patch example_config.toml -o example_config_oidc.toml < example_config_oidc.toml.patch
    patch example_config.toml -o example_config_external.toml < example_config_external.toml.patch

update_file_snapshots:
    #!/usr/bin/env sh
    set -e
    parallel :::\
        ./scripts/snapshot_files/systemd.sh\
        ./scripts/snapshot_files/openrc.sh\
        ./scripts/snapshot_files/compose_and_self_extracting.sh

install_test_scripts:
    ./scripts/tests/enduser_install_scripts.sh
    ./scripts/tests/direct-control-alpine.sh
    ./scripts/tests/direct-control-pwsh.sh
    ./scripts/tests/direct-control-ubuntu.sh
    ./scripts/tests/service-installation-openrc.sh
    ./scripts/tests/service-installation-systemd.sh

alias deny := ci_cargo_deny

ci_typo:
    typos

playwright +flags="":
    cd frontend && npm ci && npx tsc --noEmit && npx playwright test {{flags}}

playwright_report:
    cd frontend && npx playwright show-report

release TYPE skip_updates="false":
    #!/usr/bin/env bash
    set -e
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
    echo "Starting {{TYPE}} release process..."
    if [ "{{skip_updates}}" != "true" ]; then
        just update_dependencies
        cargo fmt
        just update_test_config_diffs
        just patch_test_configs
        just update_file_snapshots
        just ci_cargo_deny
        just db_update_sqlx_cache
        just coverage
    else
        echo "Skipping pre-release updates..."
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
