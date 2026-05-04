#!/usr/bin/env bash
set -euo pipefail

# Release script for grite
# Bumps version across all packaging formats, commits, tags, optionally publishes,
# and pushes to trigger CI/CD.
#
# Usage:
#   ./scripts/release.sh --bump patch          # 0.3.0 -> 0.3.1
#   ./scripts/release.sh --bump minor          # 0.3.0 -> 0.4.0
#   ./scripts/release.sh --bump major          # 0.3.0 -> 1.0.0
#   ./scripts/release.sh --version 0.4.0       # explicit version
#   ./scripts/release.sh --bump patch --dry-run  # preview changes
#   ./scripts/release.sh --bump patch --publish  # also cargo publish locally
#   ./scripts/release.sh --bump patch --push     # push to trigger CI/CD
#   ./scripts/release.sh --bump patch --wait     # watch CI run after push

REPO="neul-labs/grite"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT_DIR}"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

error() { echo "ERROR: $*" >&2; exit 1; }
info()  { echo "INFO:  $*"; }
dry_run=false
bump=""
explicit_version=""
do_publish=false
do_push=false
do_wait=false

usage() {
    cat << 'EOF'
Usage: ./scripts/release.sh [OPTIONS]

Options:
  --bump <major|minor|patch>   Bump version component
  --version <X.Y.Z>            Set explicit version
  --dry-run                    Show what would change without modifying files
  --publish                    Run cargo publish after bump (in dependency order)
  --push                       Push commit + tag to origin after bump
  --wait                       After push, watch the CI/CD run with gh
  -h, --help                   Show this help message

Examples:
  # Preview a patch bump
  ./scripts/release.sh --bump patch --dry-run

  # Bump minor, publish crates, push, and watch CI
  ./scripts/release.sh --bump minor --publish --push --wait

  # Explicit version, publish and push
  ./scripts/release.sh --version 0.4.0 --publish --push
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --bump)
                if [[ $# -lt 2 ]]; then
                    error "--bump requires an argument (major, minor, or patch)"
                fi
                bump="$2"
                if [[ "$bump" != "major" && "$bump" != "minor" && "$bump" != "patch" ]]; then
                    error "--bump must be major, minor, or patch"
                fi
                shift 2
                ;;
            --version)
                if [[ $# -lt 2 ]]; then
                    error "--version requires an argument (e.g., 0.4.0)"
                fi
                explicit_version="$2"
                shift 2
                ;;
            --dry-run)
                dry_run=true
                shift
                ;;
            --publish)
                do_publish=true
                shift
                ;;
            --push)
                do_push=true
                shift
                ;;
            --wait)
                do_wait=true
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                ;;
        esac
    done
}

get_current_version() {
    grep '^version = ' Cargo.toml | head -1 | sed -E 's/version = "([^"]+)"/\1/'
}

get_latest_gh_release() {
    gh release view --json tagName -q '.tagName' 2>/dev/null | sed 's/^v//' || echo "none"
}

bump_version() {
    local current="$1" component="$2"
    local major minor patch
    IFS='.' read -r major minor patch <<< "$current"
    case "$component" in
        major) echo "$((major + 1)).0.0" ;;
        minor) echo "${major}.$((minor + 1)).0" ;;
        patch) echo "${major}.${minor}.$((patch + 1))" ;;
    esac
}

confirm_version() {
    local current="$1" new="$2"
    echo ""
    echo "Current version: $current"
    echo "New version:     $new"
    if $dry_run; then
        return 0
    fi
    read -r -p "Continue? [y/N] " response
    [[ "$response" =~ ^[Yy]$ ]] || exit 0
}

run_or_dry() {
    if $dry_run; then
        echo "[DRY-RUN] would run: $*"
    else
        "$@"
    fi
}

# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------

validate() {
    # Must be on main
    local branch
    branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$branch" != "main" ]]; then
        error "Must be on main branch (currently on $branch)"
    fi

    # Working tree must be clean
    if ! git diff-index --quiet HEAD --; then
        error "Working tree is not clean. Commit or stash changes first."
    fi

    # gh must be installed and authenticated
    if ! command -v gh &> /dev/null; then
        error "gh (GitHub CLI) is required. Install from https://cli.github.com/"
    fi

    if ! gh auth status &> /dev/null; then
        error "gh is not authenticated. Run: gh auth login"
    fi

    # Verify we're in the right repo
    local remote_url
    remote_url=$(git remote get-url origin 2>/dev/null || true)
    if [[ -n "$remote_url" && "$remote_url" != *"$REPO"* ]]; then
        error "Origin remote does not point to $REPO ($remote_url)"
    fi
}

# ---------------------------------------------------------------------------
# File updates
# ---------------------------------------------------------------------------

update_file() {
    local file="$1" pattern="$2" replacement="$3"
    if $dry_run; then
        echo "[DRY-RUN] $file: replace '$pattern' with '$replacement'"
    else
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' -E "s#${pattern}#${replacement}#g" "$file"
        else
            sed -i -E "s#${pattern}#${replacement}#g" "$file"
        fi
    fi
}

update_versions() {
    local old="$1" new="$2"

    info "Updating version references: $old → $new"

    # Root Cargo.toml (workspace version)
    update_file "Cargo.toml" "^version = \"${old}\"" "version = \"${new}\""

    # Internal crate dependency versions
    for crate_toml in crates/*/Cargo.toml; do
        update_file "$crate_toml" "version = \"${old}\"" "version = \"${new}\""
    done

    # npm package.json
    update_file "packaging/npm/package.json" "\"version\": \"${old}\"" "\"version\": \"${new}\""

    # pip pyproject.toml
    update_file "packaging/pip/pyproject.toml" "^version = \"${old}\"" "version = \"${new}\""

    # pip __init__.py
    update_file "packaging/pip/grite_cli/__init__.py" "__version__ = \"${old}\"" "__version__ = \"${new}\""

    # gem gemspec
    update_file "packaging/gem/grite-cli.gemspec" "spec\.version *= *'${old}'" "spec.version       = '${new}'"

    # gem ruby file
    update_file "packaging/gem/lib/grite-cli.rb" "VERSION = '${old}'" "VERSION = '${new}'"

    # chocolatey nuspec
    update_file "packaging/chocolatey/grite.nuspec" "<version>${old}</version>" "<version>${new}</version>"

    # chocolatey install script
    update_file "packaging/chocolatey/tools/chocolateyinstall.ps1" "\$version = '${old}'" "\$version = '${new}'"

    # homebrew formula
    update_file "packaging/homebrew/grite.rb" "version \"${old}\"" "version \"${new}\""
}

update_changelog() {
    local new_version="$1"
    local today
    today=$(date +%Y-%m-%d)

    if $dry_run; then
        echo "[DRY-RUN] CHANGELOG.md: add date to [Unreleased] and create new [Unreleased]"
        return 0
    fi

    # Add date to current [Unreleased] section
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s/## \[Unreleased\]/## [Unreleased]\n\n## [${new_version}] - ${today}/" CHANGELOG.md
    else
        sed -i "s/## \[Unreleased\]/## [Unreleased]\n\n## [${new_version}] - ${today}/" CHANGELOG.md
    fi

    info "Updated CHANGELOG.md with release date $today"
}

# ---------------------------------------------------------------------------
# Cargo operations
# ---------------------------------------------------------------------------

cargo_dry_run() {
    if $dry_run; then
        echo "[DRY-RUN] cargo publish --dry-run for all crates"
        return 0
    fi
    local crates=("libgrite-core" "libgrite-git" "libgrite-ipc" "libgrite-cli" "grite" "grite-daemon")
    for crate in "${crates[@]}"; do
        info "Dry-run publishing $crate..."
        cargo publish --dry-run --package "$crate"
    done
}

cargo_publish() {
    if $dry_run; then
        echo "[DRY-RUN] cargo publish for all crates (with 30s delays)"
        return 0
    fi
    local crates=("libgrite-core" "libgrite-git" "libgrite-ipc" "libgrite-cli" "grite" "grite-daemon")
    for crate in "${crates[@]}"; do
        info "Publishing $crate..."
        cargo publish --package "$crate"
        info "Waiting 30s for crates.io index..."
        sleep 30
    done
}

# ---------------------------------------------------------------------------
# Git operations
# ---------------------------------------------------------------------------

git_commit_and_tag() {
    local version="$1"
    if $dry_run; then
        echo "[DRY-RUN] git add -A && git commit -m 'chore(release): bump version to $version'"
        echo "[DRY-RUN] git tag v$version"
        return 0
    fi

    info "Committing version bump..."
    git add -A
    git commit -m "chore(release): bump version to $version"

    info "Tagging v$version..."
    git tag "v$version"
}

git_push() {
    if $dry_run; then
        echo "[DRY-RUN] git push origin main"
        echo "[DRY-RUN] git push origin --tags"
        return 0
    fi

    info "Pushing to origin..."
    git push origin main
    git push origin --tags
    info "Pushed. CI/CD will build binaries and publish to registries."
}

watch_ci() {
    if $dry_run; then
        echo "[DRY-RUN] gh run watch --workflow=release.yml"
        return 0
    fi

    info "Waiting for CI run to start..."
    sleep 5

    local run_id
    run_id=$(gh run list --workflow=release.yml --limit=1 --json databaseId -q '.[0].databaseId')
    if [[ -z "$run_id" ]]; then
        error "Could not find CI run. Check manually with: gh run list --workflow=release.yml"
    fi

    info "Watching CI run $run_id..."
    gh run watch "$run_id"

    local conclusion
    conclusion=$(gh run view "$run_id" --json conclusion -q '.conclusion')
    if [[ "$conclusion" == "success" ]]; then
        info "CI/CD completed successfully!"
        local release_url
        release_url=$(gh release view "v$1" --json url -q '.url' 2>/dev/null || true)
        if [[ -n "$release_url" ]]; then
            info "Release: $release_url"
        fi
    else
        error "CI/CD failed with conclusion: $conclusion"
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    parse_args "$@"

    if [[ -z "$bump" && -z "$explicit_version" ]]; then
        error "Must specify either --bump or --version"
    fi

    validate

    local current_version new_version
    current_version=$(get_current_version)
    new_version="${explicit_version:-$(bump_version "$current_version" "$bump")}"

    confirm_version "$current_version" "$new_version"

    info "Starting release $new_version..."

    update_versions "$current_version" "$new_version"
    update_changelog "$new_version"

    # Regenerate Cargo.lock
    if ! $dry_run; then
        info "Regenerating Cargo.lock..."
        cargo check --workspace --quiet
    fi

    git_commit_and_tag "$new_version"

    if $do_publish; then
        cargo_dry_run
        cargo_publish
    elif ! $dry_run; then
        info "Skipping cargo publish. Use --publish to publish crates locally."
    fi

    if $do_push; then
        git_push

        if $do_wait; then
            watch_ci "$new_version"
        else
            info "Monitor CI/CD with: gh run list --workflow=release.yml"
        fi
    elif ! $dry_run; then
        info "Skipping push. Use --push to trigger CI/CD."
    fi

    info "Done."
}

main "$@"
