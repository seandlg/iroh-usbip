## Agent skills

### Issue tracker

Issues are tracked on GitHub (uses the `gh` CLI). External PRs are not treated as a request surface. See [issue-tracker.md](docs/agents/issue-tracker.md).

### Triage labels

Using standard triage labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See [triage-labels.md](docs/agents/triage-labels.md).

### Domain docs

Single-context repository layout (one `CONTEXT.md` + `docs/adr/` at the root). See [domain.md](docs/agents/domain.md).

### Commits and Issue Linking

Always use Conventional Commits for commit messages and link any issue currently being addressed.
- **Commit format**: `<type>(<optional scope>): <description> (fixes #<issue_number>)`
- **Allowed Types**: `feat`, `fix`, `refactor`, `docs`, `chore`, `test`, `impl`
- **Rule**: Whenever you are asked to commit or draft a commit message, you MUST append `(fixes #<number>)` referencing the issue number if one exists.
- **Examples**:
  - `feat: task runner integration and nix flake check setup (fixes #12)`
  - `refactor(protocol): restructure transfer phase to typed Rust structs (fixes #8)`
  - `docs: update README.md with nix/cargo interplay instructions`


### Release Process

Releases are fully automated and mistake-proof (Poka-Yoke). If you are asked to prepare or tag a release, you MUST use the task runner recipes in [justfile](file:///Users/river/Coding/iroh-usbip/justfile).
- **Rule**: Never create or push git tags manually. Always run `just tag-release` from `main`, which automatically verifies CI success via `gh`.
- **Rule**: Never run `prepare-release` on a dirty branch or directly on `main` without pulling. Always run `just prepare-release <version>` from a clean `main` branch.
- **Rule**: Follow SemVer strictly. Determine the version bump type (major, minor, patch) based on the Conventional Commits since the last release tag.


