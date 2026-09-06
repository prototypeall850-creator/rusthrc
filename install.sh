#!/usr/bin/env bash
# rusthrc installer for macOS, Linux and Termux.
#
#   curl -fsSL https://raw.githubusercontent.com/prototypeall850-creator/rusthrc/main/install.sh | bash
#   curl -fsSL ... | bash -s -- v0.1.0        # pin a version
#   RUSTHRC_VERSION=v0.1.0 RUSTHRC_INSTALL_DIR=~/.local/bin bash <(curl -fsSL ...)
#
# macOS and Linux get prebuilt static binaries from GitHub Releases.
# Termux has no prebuilt Android binaries — the script builds from source.
set -eu

REPO="prototypeall850-creator/rusthrc"
VERSION="${RUSTHRC_VERSION:-${1:-latest}}"
INSTALL_DIR="${RUSTHRC_INSTALL_DIR:-$HOME/.local/bin}"

log() { printf '\033[1;36m·\033[0m %s\n' "$*"; }
err() { printf '\033[1;31m✗ error:\033[0m %s\n' "$*" >&2; exit 1; }

command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 \
  || err "curl or wget is required"

fetch() { # fetch <url> <output-file>
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1" -o "$2"
  else
    wget -qO "$2" "$1"
  fi
}

is_termux() {
  [ -n "${TERMUX_VERSION:-}" ] && return 0
  case "${PREFIX:-}" in *com.termux*) return 0 ;; esac
  return 1
}

resolve_tag() {
  if [ "$VERSION" != "latest" ]; then
    case "$VERSION" in
      v*) TAG="$VERSION" ;;
      *) TAG="v$VERSION" ;;
    esac
    return
  fi
  log "resolving the latest release…"
  api=$(fetch "https://api.github.com/repos/$REPO/releases/latest" /dev/stdout) \
    || err "cannot reach the GitHub API"
  TAG=$(printf '%s' "$api" | grep -o '"tag_name": *"[^"]*"' | head -1 | sed 's/.*"tag_name": *"//;s/"$//')
  [ -n "$TAG" ] || err "could not determine the latest release — set RUSTHRC_VERSION explicitly"
}

sha256_of() { # sha256_of <file>
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

install_from_source() {
  log "Termux detected — building from source (a few minutes, one time only)"
  command -v pkg >/dev/null 2>&1 || err "pkg not found; update Termux first (pkg update)"
  pkg install -y rust git clang openssl pkg-config
  resolve_tag
  log "building rusthrc $TAG with cargo…"
  cargo install --locked --git "https://github.com/$REPO" rusthrc --tag "$TAG"
  BIN_SRC="${CARGO_HOME:-$HOME/.cargo}/bin/rusthrc"
  [ -x "$BIN_SRC" ] || err "build finished but the binary is not at $BIN_SRC"
  mkdir -p "${PREFIX:-/data/data/com.termux/files/usr}/bin"
  ln -sf "$BIN_SRC" "${PREFIX:-/data/data/com.termux/files/usr}/bin/rusthrc"
  log "installed to \$PREFIX/bin/rusthrc"
  rusthrc --version
  exit 0
}

install_prebuilt() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os" in
    Darwin) os_part="apple-darwin" ;;
    Linux) os_part="unknown-linux-musl" ;;
    *) err "unsupported OS: $os (Windows: use install.ps1)" ;;
  esac
  case "$arch" in
    x86_64 | amd64) arch_part="x86_64" ;;
    aarch64 | arm64) arch_part="aarch64" ;;
    *) err "unsupported architecture: $arch — build from source with 'cargo install --git https://github.com/$REPO'" ;;
  esac
  TARGET="${arch_part}-${os_part}"

  resolve_tag
  VER="${TAG#v}"
  ASSET="rusthrc-${VER}-${TARGET}.tar.gz"
  URL="https://github.com/$REPO/releases/download/$TAG/$ASSET"

  TMP=$(mktemp -d)
  trap 'rm -rf "$TMP"' EXIT

  log "downloading $ASSET…"
  fetch "$URL" "$TMP/$ASSET" || err "download failed: $URL"
  if fetch "$URL.sha256" "$TMP/$ASSET.sha256" 2>/dev/null && [ -s "$TMP/$ASSET.sha256" ]; then
    expected=$(cut -d' ' -f1 "$TMP/$ASSET.sha256" | tr -d '[:space:]')
    actual=$(sha256_of "$TMP/$ASSET")
    [ "$expected" = "$actual" ] || err "checksum mismatch for $ASSET"
    log "checksum verified"
  fi

  tar xzf "$TMP/$ASSET" -C "$TMP"
  BIN="$TMP/rusthrc-${VER}-${TARGET}/rusthrc"
  [ -x "$BIN" ] || err "unexpected archive layout"

  mkdir -p "$INSTALL_DIR"
  mv "$BIN" "$INSTALL_DIR/rusthrc"
  chmod +x "$INSTALL_DIR/rusthrc"
  log "installed $INSTALL_DIR/rusthrc"

  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) log "NOTE: $INSTALL_DIR is not in PATH — add this to your shell profile:"
       log "      export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
  esac
  "$INSTALL_DIR/rusthrc" --version
}

if is_termux; then
  install_from_source
else
  install_prebuilt
fi
