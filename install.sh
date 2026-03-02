#!/usr/bin/env bash
set -euo pipefail

REPO="crowecawcaw/percept"
BINARY="percept"
INSTALL_DIR="${PERCEPT_INSTALL_DIR:-/usr/local/bin}"

info() { printf '\033[1;34m%s\033[0m\n' "$*"; }
error() { printf '\033[1;31merror: %s\033[0m\n' "$*" >&2; exit 1; }

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)  os="unknown-linux-gnu" ;;
    Darwin) os="apple-darwin" ;;
    *)      error "Unsupported OS: $os" ;;
  esac

  case "$arch" in
    x86_64|amd64)   arch="x86_64" ;;
    aarch64|arm64)   arch="aarch64" ;;
    *)               error "Unsupported architecture: $arch" ;;
  esac

  echo "${arch}-${os}"
}

get_latest_version() {
  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' \
    | head -1 \
    | sed 's/.*"tag_name": *"//;s/".*//'
}

main() {
  info "Detecting platform..."
  local target version url tmpdir

  target="$(detect_target)"
  info "Target: ${target}"

  if [ -n "${PERCEPT_VERSION:-}" ]; then
    version="$PERCEPT_VERSION"
  else
    info "Fetching latest release..."
    version="$(get_latest_version)"
  fi

  [ -z "$version" ] && error "Could not determine latest version. Set PERCEPT_VERSION to install a specific version."
  info "Version: ${version}"

  url="https://github.com/${REPO}/releases/download/${version}/${BINARY}-${target}.tar.gz"
  info "Downloading ${url}..."

  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  curl -fSL "$url" -o "${tmpdir}/${BINARY}.tar.gz"

  info "Verifying checksum..."
  local checksums_url="https://github.com/${REPO}/releases/download/${version}/checksums-sha256.txt"
  if curl -fsSL "$checksums_url" -o "${tmpdir}/checksums-sha256.txt" 2>/dev/null; then
    expected="$(grep "${BINARY}-${target}.tar.gz" "${tmpdir}/checksums-sha256.txt" | awk '{print $1}')"
    if [ -n "$expected" ]; then
      actual="$(sha256sum "${tmpdir}/${BINARY}.tar.gz" 2>/dev/null || shasum -a 256 "${tmpdir}/${BINARY}.tar.gz" | awk '{print $1}')"
      actual="$(echo "$actual" | awk '{print $1}')"
      if [ "$expected" != "$actual" ]; then
        error "Checksum mismatch! Expected: ${expected}, Got: ${actual}"
      fi
      info "Checksum verified."
    fi
  else
    info "Checksums not available, skipping verification."
  fi

  info "Extracting..."
  tar xzf "${tmpdir}/${BINARY}.tar.gz" -C "${tmpdir}"

  info "Installing to ${INSTALL_DIR}..."
  if [ -w "$INSTALL_DIR" ]; then
    mv "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
  else
    sudo mv "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
  fi
  chmod +x "${INSTALL_DIR}/${BINARY}"

  info "percept ${version} installed to ${INSTALL_DIR}/${BINARY}"
  info "Run 'percept setup' to download the required ML models."
}

main
