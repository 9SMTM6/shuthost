# this ensures that just running just give a list of commands
[private]
list:
    just --list

choose:
    just --choose

alias c := choose

mod frontend
mod coordinator
mod examples 'docs/examples'
mod scripts

alias build_graphs := frontend::build_graphs
alias playwright := frontend::playwright
alias pixelpeep := frontend::pixelpeep
alias playwright_report := frontend::playwright_report
alias db_create := coordinator::db_create
alias db_update_sqlx_cache := coordinator::db_update_sqlx_cache
alias db_add_migration := coordinator::db_add_migration
alias update_test_config_diffs := examples::update_test_config_diffs
alias patch_test_configs := examples::patch_test_configs
alias build_gh_pages := scripts::build_gh_pages
alias tsc := frontend::typecheck

install_test_scripts:
    just scripts::installer_tests

update_file_snapshots:
    just scripts::update_all_snapshots

export RUST_BACKTRACE := "1"
export LOG_FORMAT := "pretty"

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

[group('devops')]
[confirm]
clean:
    cargo clean && cargo fetch
    just frontend::clean

[group('projectmanagement')]
update_dependencies:
    cargo update --verbose
    just frontend::update_deps

alias deps := update_dependencies

export DATABASE_URL := "sqlite:" + justfile_directory() + "/shuthost.db"
export SQLX_OFFLINE := "true"

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
    cd frontend && pnpm run test && cd ..
    just build_gh_pages --provided-binary=target/debug/shuthost_coordinator
    cargo nextest run --workspace
    cd {{justfile_directory()}}
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

[group('tests')]
typos:
    typos

alias typo := typos

[group('tests')]
cargo_clippy +flags="":
    cargo clippy --workspace --all-targets {{flags}}

alias clippy := cargo_clippy

[group('projectmanagement')]
rust_fmt:
    cargo fmt --all

[group('projectmanagement')]
rust_clippy_fix +flags="":
    __CARGO_FIX_YOLO=1 cargo clippy --fix --workspace --all-targets --allow-dirty {{flags}}

[group('projectmanagement')]
rustfix_yolo:
    just rust_clippy_fix
    just fmt

[group('projectmanagement')]
yolo:
    just rustfix_yolo
    just frontend::yolo

[group('projectmanagement')]
fmt:
    just rust_fmt
    just frontend::fmt

[script]
[group('tests')]
cargo_tests +flags="":
    RUST_BACKTRACE=0 cargo nextest run --workspace {{flags}}

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
    just rust_fmt
    just update_test_config_diffs
    just patch_test_configs
    just cargo_deny
    just frontend::typecheck
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
    git add .
    while true; do
        read -p "Please check the staged changes. Continue with commit? (y/N/s for shell) " -n 1 -r REPLY
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

    git commit -m "Create Release $NEW_VERSION" -m "Automated release tasks performed:" -m "* Updated dependencies" -m "* Formatted code with cargo fmt" -m "* Bumped version to $NEW_VERSION" -m "* Updated config patches" -m "* Updated file snapshots" -m "* Updated sqlx cache (if required)" -m "" -m "## Changes not tracked in a PR:" -m "$MANUAL_CHANGES"
    git tag "$NEW_VERSION"
    # we need separate pushes here to ensure that the tag push triggers the *release-tag jobs
    git push origin refs/heads/main
    git push origin "$NEW_VERSION"
    echo "{{TYPE}} release $NEW_VERSION completed successfully!"
