# Replace pnpm with aube (Rust Embedding)

**Status: Rejected — document for future re-evaluation**

## Context

pnpm is currently a hard build dependency for the project. It is used in three
contexts:

1. **Rust build script** (`coordinator/build/`) — spawned as a child process
   to install frontend dependencies, run package scripts (typecheck, build,
   build:diagrams, build:prerender), and collect npm package license information.
2. **CI** (`.github/workflows/`) — via `pnpm/action-setup@v5` and direct
   `pnpm` invocations for install, lint, typecheck, and Playwright tests.
3. **Developer tooling** (Justfiles, `scripts/build.Containerfile`,
   `docs/CONTRIBUTING.md`) — all `pnpm` commands for local development.

[aube](https://aube.jdx.dev) is a Node.js package manager with a native Rust
embedding API (`aube::embed`). This document evaluates embedding aube as a
Cargo dependency to eliminate pnpm from the Rust build script's external
dependency chain.

A key design constraint if this were pursued: pnpm config files
(`pnpm-workspace.yaml`, `pnpm-lock.yaml`) stay as-is. Contributors who prefer
pnpm for day-to-day frontend work would continue using it — aube maintains its
own package store and steps in only when the Rust build script runs.

---

## What the Embedding API Covers

| Operation | pnpm (current) | aube embedding |
|---|---|---|
| `install --frozen-lockfile` | `std::process::Command` | `embed::install()` |
| `run <script>` | `std::process::Command` | `embed::run(project_dir, script, ...)` |
| `exec <binary>` | `std::process::Command` | `embed::exec(project_dir, binary, ...)` |
| `licenses list --json` | `std::process::Command` | **No embedding API** |
| `--version` check | `std::process::Command` | Not needed (compile-time dep) |
| Error handling | stderr parsing, exit codes | `ERR_AUBE_*` codes, `miette` diagnostics |

---

## Downsides (reasons against, with outlook for change)

### 1. `aube::embed` has no licenses API

The `process_pnpm_packages` function in `coordinator/build/about.rs:211-263`
shells out to `pnpm licenses list --json` to collect name, version, license,
author, and homepage URL for every npm dependency. The embedding API has no
equivalent — `aube licenses --json` is CLI-only and its output format differs
from pnpm's (flat array instead of grouped by SPDX key, no homepage field).

Fixing this in the build script would require rewriting `about.rs` to collect
dependency metadata by walking `node_modules` and reading `package.json`
files directly, handling aube's virtual store layout (`node_modules/.aube/`).
This is independent of aube's embedding API quality — it is work that needs
doing regardless of how aube evolves.

**Likelihood of change:** Low. aube would need to either expose an embedding
licenses API or extend its CLI output to include homepage URLs and match
pnpm's grouping. Neither is on the roadmap as of this writing.

### 2. Build script must bridge sync↔async

The build script (`coordinator/build/mod.rs`) is synchronous. The `aube::embed`
API is async (returns `Future`). Every call would need either a
`tokio::runtime::Runtime::block_on` wrapper or a structural refactor to share
one runtime across all aube operations.

**Likelihood of change:** Low. aube's embedding API is fundamentally async
(dependency resolution is I/O-bound). A sync wrapper is unlikely. The
build script could reasonably adopt a shared runtime, but it adds ceremony.

### 3. Two package managers touching the same project

aube and pnpm would share `node_modules` and `pnpm-lock.yaml`. In theory they
read/write the same formats and are interoperable. In practice, concurrent use
of two package managers on the same project carries a risk of subtle state
divergence (different virtual store layouts, different resolution semantics
for edge cases).

**Likelihood of change:** Low for the coexistence scenario itself (it is
inherent to the design constraint). aube's lockfile compatibility could
improve with wider adoption, reducing divergence risk.

### 4. Larger build-dependency tree

aube brings its own dependency graph (Tokio, reqwest, etc.) into
`[build-dependencies]`. This increases compile time for the coordinator crate
and its dependents.

**Likelihood of change:** Low. aube is a package manager — it needs an HTTP
client and an async runtime. The dependency footprint is structural.

### 5. Platform-specific binary downloads

With `default-features = false`, aube avoids downloading native binaries at
crate build time, but this code path is less exercised than the CLI. Edge
cases in cross-compilation or unusual targets are more likely.

**Likelihood of change:** Medium. As `aube::embed` gains adoption, the
library path will receive more real-world testing. Over time this risk
diminishes.

### 6. Young embedding API

aube v1 was released recently. The `aube::embed` facade is documented as
stable, but the `aube::commands` modules (which the embedding API replaces)
are explicitly described as not the preferred API for native hosts. The
boundary between the two may shift.

**Likelihood of change:** Medium. The embedding API is explicitly designed for
this use case, so it should stabilise. Breaking changes in the minor version
would be a trust issue for the project.

---

## License Collection Gap Detail

Of all the downsides, this is the one that requires the most work regardless
of aube's evolution. The current code:

```
coordinator/build/about.rs:211-263 — process_pnpm_packages()
  shells out to: pnpm licenses list --json
  parses: HashMap<String, Vec<{name, versions, license, author?, homepage?}>>
  writes: combined Vec<CombinedEntry> → about-data.json
```

If aube were adopted, `about.rs` would need to be rewritten to collect the
same data without a `licenses` CLI subprocess. Options:

| Option | Effort | Quality |
|---|---|---|
| Walk `node_modules/.aube/` symlinks and read `package.json` files | Medium | Equal (same fields available) |
| Parse `pnpm-lock.yaml` directly (aube writes it) | Medium | Lower (lockfile has no author/homepage) |
| Keep pnpm only for license generation | Low | Lower (defeats the purpose) |

The `node_modules` walk is the best path if this is ever revisited.

---

## Verdict

Not worth pursuing now. The license collection gap requires non-trivial build
script work independent of aube, and the other downsides (async bridge,
coexistence risk, dependency weight) are inherent to the approach rather than
maturity issues that time will solve. Re-evaluate if aube adds an embedding
licenses API or if the build script's license collection is otherwise
rewritten (making the gap moot).
