# iroh-usbip task runner
#
# Standardizes development tasks, wrapping commands with nix develop.

# Run clippy and format check inside the Nix environment
check:
    nix develop --command cargo fmt --all --check
    nix develop --command cargo clippy --all-targets -- --deny warnings

# Build the binaries using Nix
build:
    nix build

# Run the unit and mock integration tests inside the Nix environment
test:
    nix develop --command cargo test

# Run the E2E integration test on Linux (requires sudo/root privileges)
test-e2e:
    sudo nix develop --command scripts/e2e.sh

# Prepare a new release (run locally from main branch)
prepare-release version:
    @if [ "$(git branch --show-current)" != "main" ]; then echo "Error: prepare-release must be run on the main branch."; exit 1; fi
    @if [ -n "$(git status --porcelain)" ]; then echo "Error: git working directory is not clean."; exit 1; fi
    git pull origin main
    @echo "Checking CI status for the latest commit on main (Gate 1)..."
    @COMMIT_SHA=$(git rev-parse HEAD); \
     CI_STATUS=$(gh run list --commit $$COMMIT_SHA --json status,conclusion --jq '.[0] | "\(.status) \(.conclusion)"' 2>/dev/null); \
     if [ "$$CI_STATUS" != "completed success" ]; then \
         echo "Error: CI status for commit $$COMMIT_SHA is: $${CI_STATUS:-no run found}."; \
         echo "Gate 1 failed: You can only prepare a release from a successful CI build on main."; \
         echo "Check CI runs at: https://github.com/seandlg/iroh-usbip/actions"; \
         exit 1; \
     fi
    git checkout -b release/v{{version}}
    python3 -c "import re; p = open('Cargo.toml').read(); p = re.sub(r'(?m)^version = \".*?\"', 'version = \"{{version}}\"', p, 1); open('Cargo.toml', 'w').write(p)"
    nix develop --command cargo check
    nix run nixpkgs#git-cliff -- --tag v{{version}} --prepend CHANGELOG.md
    git add Cargo.toml Cargo.lock CHANGELOG.md
    git commit -m "chore: release {{version}}"
    @echo ""
    @echo "Release branch release/v{{version}} prepared successfully!"
    @echo "Please push this branch, open a PR, and merge it when CI passes."

# Tag and trigger release (run locally on main branch after release PR is merged)
tag-release:
    @if [ "$(git branch --show-current)" != "main" ]; then echo "Error: tag-release must be run on the main branch."; exit 1; fi
    @if [ -n "$(git status --porcelain)" ]; then echo "Error: git working directory is not clean."; exit 1; fi
    git pull origin main
    @echo "Checking CI status for the merged release commit (Gate 2)..."
    @COMMIT_SHA=$(git rev-parse HEAD); \
     CI_STATUS=$(gh run list --commit $$COMMIT_SHA --json status,conclusion --jq '.[0] | "\(.status) \(.conclusion)"' 2>/dev/null); \
     if [ "$$CI_STATUS" != "completed success" ]; then \
         echo "Error: CI status for merged release commit $$COMMIT_SHA is: $${CI_STATUS:-no run found}."; \
         echo "Gate 2 failed: CI for the merged release commit must succeed before tagging."; \
         echo "Check CI runs at: https://github.com/seandlg/iroh-usbip/actions"; \
         exit 1; \
     fi
    @VERSION=$(python3 -c "import re; print(re.search(r'(?m)^version = \"(.*?)\"', open('Cargo.toml').read()).group(1))"); \
     git tag -a v$$VERSION -m "Release v$$VERSION"; \
     git push origin v$$VERSION; \
     echo "Successfully tagged and pushed v$$VERSION!"
