# ADR 0010: Release Automation with cargo-dist and git-cliff

## Context

We need a simple, reliable, and reproducible release process to package, version, and distribute `iroh-usbip` binaries. To implement this, we evaluated options for packaging (e.g., custom CI scripts vs. `cargo-dist`), changelog generation (manual vs. `git-cliff`), and tagging safety (direct push vs. CI-gated checks).

Additionally, we had to address two critical constraints:
1. The project has system dependencies (`libusb`), which complicates dynamic linking and cross-compilation on different platforms.
2. E2E tests cannot run locally on macOS or without root privileges on Linux, making local pre-tagging verification difficult.

## Decision

We decided to adopt the following architecture and workflow:

1. **`cargo-dist` for Packaging & CI**: We use `cargo-dist` to automatically generate `.github/workflows/release.yml` and manage the multi-platform build matrix (Linux `x86_64`/`aarch64`, macOS `x86_64`/`aarch64`, Windows `x86_64`) on tag triggers (`v*`).
2. **`git-cliff` for Changelogs**: We use `git-cliff` locally to maintain `CHANGELOG.md` based on Conventional Commits. `cargo-dist` consumes the resulting `CHANGELOG.md` file in CI to extract the latest release notes and populate the GitHub Release.
3. **Double-Gated Poka-Yoke CI Verification**:
   - **Gate 1 (Prepare)**: Before preparing a release branch locally with `just prepare-release <version>`, we query GitHub Actions via `gh` to verify that the latest commit on `main` passed CI.
   - **Gate 2 (Tag)**: Before tagging the release commit locally with `just tag-release`, we verify that the merged release PR commit on `main` passed CI.
4. **Static Linking via Vendored Rusb**: We enable the `vendored` feature on `rusb` statically in `Cargo.toml` to compile `libusb` from source.

## Consequences

*   **Self-Contained Binaries:** The compiled binaries are fully self-contained and do not require users to install `libusb` at runtime.
*   **Zero CI Upkeep:** The release CI workflow is generated and updated declaratively via `cargo-dist`.
*   **Mistake-Proof (Poka-Yoke) Release Flow:** You cannot tag or prepare a release on top of broken commits.
*   **Fail-Fast Safety:** Checks fail-fast immediately if CI has not succeeded yet, preventing slow "polling/waiting" loops.
*   **Binary Size / Security Update Trade-off:** The binaries are slightly larger (~100-200 KB) and future security updates to `libusb` will require a rebuild and re-release of `iroh-usbip`.
