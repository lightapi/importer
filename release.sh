#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./release.sh <tag>

Build release archives for Linux and macOS Apple Silicon, then create or update
the matching GitHub release and upload the assets.

Environment:
  REPO=owner/name              GitHub repo. Defaults to origin remote.
  DIST_DIR=dist                Output directory.
  LINUX_TARGET=...             Default: x86_64-unknown-linux-gnu.
  MACOS_TARGET=...             Default: aarch64-apple-darwin.
  BUILD_LINUX=0               Skip Linux build.
  BUILD_MACOS=0               Skip macOS Apple Silicon build.
  UPLOAD=0                    Build archives/checksums but skip gh release upload.
  ALLOW_DIRTY=1               Allow releasing with uncommitted changes.

Cross-platform notes:
  - Native Linux builds use cargo.
  - Native macOS builds use cargo.
  - Cross builds use cargo-zigbuild when available.
  - Building aarch64-apple-darwin from Linux requires zig and cargo-zigbuild.
  - To publish Linux only from Linux, run BUILD_MACOS=0 ./release.sh <tag>.
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

TAG="${1:-}"
if [[ -z "$TAG" ]]; then
  usage
  exit 2
fi

BIN_NAME="importer"
DIST_DIR="${DIST_DIR:-dist}"
LINUX_TARGET="${LINUX_TARGET:-x86_64-unknown-linux-gnu}"
MACOS_TARGET="${MACOS_TARGET:-aarch64-apple-darwin}"
BUILD_LINUX="${BUILD_LINUX:-1}"
BUILD_MACOS="${BUILD_MACOS:-1}"

ensure_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Required command not found: $command_name" >&2
    exit 2
  fi
}

repo_from_origin() {
  local url
  url="$(git config --get remote.origin.url || true)"
  case "$url" in
    git@github.com:*)
      url="${url#git@github.com:}"
      echo "${url%.git}"
      ;;
    https://github.com/*)
      url="${url#https://github.com/}"
      echo "${url%.git}"
      ;;
    *)
      echo ""
      ;;
  esac
}

check_clean_worktree() {
  if [[ "${ALLOW_DIRTY:-0}" == "1" ]]; then
    return
  fi
  if ! git diff --quiet || ! git diff --cached --quiet || [[ -n "$(git status --porcelain)" ]]; then
    echo "Working tree is not clean. Commit/stash changes or set ALLOW_DIRTY=1." >&2
    exit 2
  fi
}

ensure_target() {
  local target="$1"
  if command -v rustup >/dev/null 2>&1; then
    if ! rustup target list --installed | grep -qx "$target"; then
      rustup target add "$target"
    fi
  fi
}

can_native_build() {
  local target="$1"
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$target" in
    x86_64-unknown-linux-gnu)
      [[ "$os" == "Linux" && "$arch" == "x86_64" ]]
      ;;
    aarch64-apple-darwin)
      [[ "$os" == "Darwin" ]]
      ;;
    *)
      return 1
      ;;
  esac
}

can_build_target() {
  local target="$1"
  can_native_build "$target" || {
    command -v cargo-zigbuild >/dev/null 2>&1 && command -v zig >/dev/null 2>&1
  }
}

preflight_targets() {
  local missing=()

  if [[ "$BUILD_LINUX" != "0" ]] && ! can_build_target "$LINUX_TARGET"; then
    missing+=("$LINUX_TARGET")
  fi
  if [[ "$BUILD_MACOS" != "0" ]] && ! can_build_target "$MACOS_TARGET"; then
    missing+=("$MACOS_TARGET")
  fi

  if [[ ${#missing[@]} -eq 0 ]]; then
    return
  fi

  echo "Cannot build requested target(s) on this host: ${missing[*]}" >&2
  echo "Install zig + cargo-zigbuild, build the missing target on a native host, or skip it." >&2
  echo "For Linux-only release from this host: BUILD_MACOS=0 ./release.sh ${TAG}" >&2
  exit 2
}

build_target() {
  local target="$1"
  local platform="$2"
  local archive_name="${BIN_NAME}-${TAG}-${platform}.tar.gz"
  local stage_dir="${DIST_DIR}/stage/${BIN_NAME}-${TAG}-${platform}"
  ensure_target "$target"

  if can_native_build "$target"; then
    echo "Building $target with cargo"
    cargo build --release --locked --target "$target"
  elif command -v cargo-zigbuild >/dev/null 2>&1 && command -v zig >/dev/null 2>&1; then
    echo "Building $target with cargo zigbuild"
    cargo zigbuild --release --locked --target "$target"
  else
    echo "Cannot build $target on this host." >&2
    echo "Install zig + cargo-zigbuild, or build this target on a native host." >&2
    exit 2
  fi

  local binary="target/${target}/release/${BIN_NAME}"
  if [[ ! -x "$binary" ]]; then
    echo "Expected binary not found: $binary" >&2
    exit 2
  fi

  rm -rf "$stage_dir"
  mkdir -p "$stage_dir"
  cp "$binary" "$stage_dir/${BIN_NAME}"
  cp README.md "$stage_dir/"
  if [[ -f LICENSE ]]; then
    cp LICENSE "$stage_dir/"
  fi
  tar -C "$stage_dir" -czf "${DIST_DIR}/${archive_name}" .
}

write_checksums() {
  (
    cd "$DIST_DIR"
    shopt -s nullglob
    local archives=(./*.tar.gz)
    shopt -u nullglob

    if [[ ${#archives[@]} -eq 0 ]]; then
      echo "No release archives were built. Check BUILD_LINUX/BUILD_MACOS." >&2
      exit 2
    fi

    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum "${archives[@]}" > SHA256SUMS
    elif command -v shasum >/dev/null 2>&1; then
      shasum -a 256 "${archives[@]}" > SHA256SUMS
    else
      echo "Required command not found: sha256sum or shasum" >&2
      exit 2
    fi
  )
}

upload_release() {
  ensure_command gh

  local repo="${REPO:-$(repo_from_origin)}"
  local repo_args=()
  if [[ -n "$repo" ]]; then
    repo_args=(--repo "$repo")
  fi

  gh auth status >/dev/null

  shopt -s nullglob
  local assets=("${DIST_DIR}"/*.tar.gz)
  shopt -u nullglob
  if [[ ${#assets[@]} -eq 0 || ! -f "${DIST_DIR}/SHA256SUMS" ]]; then
    echo "No release assets found. Build archives before uploading." >&2
    exit 2
  fi
  assets+=("${DIST_DIR}/SHA256SUMS")

  if gh release view "$TAG" "${repo_args[@]}" >/dev/null 2>&1; then
    echo "Uploading assets to existing release $TAG"
    gh release upload "$TAG" "${assets[@]}" --clobber "${repo_args[@]}"
  else
    echo "Creating release $TAG"
    gh release create "$TAG" "${assets[@]}" \
      --title "$TAG" \
      --notes "Release $TAG" \
      "${repo_args[@]}"
  fi
}

main() {
  ensure_command cargo
  ensure_command git
  ensure_command tar
  check_clean_worktree
  preflight_targets

  rm -rf "$DIST_DIR"
  mkdir -p "$DIST_DIR"

  if [[ "$BUILD_LINUX" != "0" ]]; then
    build_target "$LINUX_TARGET" "linux-x86_64"
  fi
  if [[ "$BUILD_MACOS" != "0" ]]; then
    build_target "$MACOS_TARGET" "macos-aarch64"
  fi

  write_checksums
  if [[ "${UPLOAD:-1}" == "0" ]]; then
    echo "UPLOAD=0 set; skipping GitHub release upload."
    echo "Release assets are in ${DIST_DIR}/"
    return
  fi

  upload_release

  echo "Release assets uploaded for $TAG"
}

main
