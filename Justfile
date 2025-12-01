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

build_coordinator_on_mac:
    cargo build --release --bin shuthost_coordinator --target aarch64-apple-darwin &
    cargo build --release --bin shuthost_coordinator --target x86_64-apple-darwin &
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_coordinator --target x86_64-unknown-linux-gnu &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc cargo build --release --bin shuthost_coordinator --target aarch64-unknown-linux-gnu &
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc cargo build --release --bin shuthost_coordinator --target x86_64-unknown-linux-musl &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc cargo build --release --bin shuthost_coordinator --target aarch64-unknown-linux-musl &
    wait

build_all: build_all_host_agents_on_mac build_coordinator_on_mac

build_graphs:
    dot frontend/assets/architecture.dot -Tsvg -ofrontend/assets/generated/architecture.svg
    dot frontend/assets/architecture_simplified.dot -Tsvg -ofrontend/assets/generated/architecture_simplified.svg

clean:
    cargo clean && cargo fetch
    cd frontend && rm -rf node_modules && npm ci

update_dependencies:
    cargo update --verbose
    cd frontend && npm update

export DATABASE_URL := "sqlite:./shuthost.db"

db_create:
    cargo sqlx database drop
    cargo sqlx database create
    cargo sqlx migrate run --source coordinator/migrations

db_update_sqlx_cache:
    cargo sqlx prepare --workspace

db_add_migration name:
    cd coordinator && sqlx migrate add {{name}}

test_all:
    cargo test --workspace

coverage:
    #!/usr/bin/env sh
    set -e
    export COVERAGE=1
    export SKIP_BUILD=1
    eval "$(cargo llvm-cov show-env --export-prefix)"
    cargo llvm-cov clean --workspace
    cargo build --workspace
    cd frontend && npm run test && cd ..
    cargo test --workspace --all-targets
    cargo llvm-cov report --lcov --output-path lcov.info --ignore-filename-regex "src/bin/coordinator.rs|host_agent/src/main.rs"
    cargo llvm-cov report --html --output-dir coverage --ignore-filename-regex "src/bin/coordinator.rs|host_agent/src/main.rs"

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

alias deny := ci_cargo_deny

ci_typo:
    typos

playwright +flags="":
    cd frontend && npm ci && npx tsc --noEmit && npx playwright test {{flags}}

playwright_report:
    cd frontend && npx playwright show-report

release TYPE:
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
    just update_dependencies
    cargo fmt
    just update_test_config_diffs
    just patch_test_configs
    just ci_cargo_deny
    just db_update_sqlx_cache
    just coverage
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

    git commit -m "Create Release $NEW_VERSION" -m "Automated release tasks performed:" -m "- Updated dependencies" -m "- Formatted code with cargo fmt" -m "- Bumped version to $NEW_VERSION" -m "- Updated Playwright snapshots" -m "- Updated config patches" -m "- Updated sqlx cache (if required)" -m "- Updated coverage report" -m "" -m "## Changes not tracked in a PR:" -m "$MANUAL_CHANGES"
    git tag "$NEW_VERSION"
    # we need separate pushes here to ensure that the tag push triggers the *release-tag jobs
    git push origin refs/heads/main
    git push origin "$NEW_VERSION"
    echo "{{TYPE}} release $NEW_VERSION completed successfully!"
