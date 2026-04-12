#!/bin/sh
set -eu

REPO="roamingparrot/parrotui-spotify"
BINARY="parrotui-spotify"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

main() {
    platform="$(uname -s)"
    arch="$(uname -m)"

    case "${platform}" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      err "Unsupported platform: ${platform}" ;;
    esac

    case "${arch}" in
        x86_64|amd64)   arch="x86_64" ;;
        arm64|aarch64)   arch="aarch64" ;;
        *)               err "Unsupported architecture: ${arch}" ;;
    esac

    target="${arch}-${os}"

    if [ "${os}" = "unknown-linux-gnu" ] && [ "${arch}" = "aarch64" ]; then
        err "Linux ARM builds are not yet available. Please build from source."
    fi

    latest="$(get_latest_tag)"
    url="https://github.com/${REPO}/releases/download/${latest}/${BINARY}-${target}.tar.gz"

    printf "Installing %s %s (%s)\n" "${BINARY}" "${latest}" "${target}"

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "${tmpdir}"' EXIT

    printf "Downloading %s\n" "${url}"
    if command -v curl > /dev/null 2>&1; then
        curl -fsSL "${url}" -o "${tmpdir}/archive.tar.gz"
    elif command -v wget > /dev/null 2>&1; then
        wget -qO "${tmpdir}/archive.tar.gz" "${url}"
    else
        err "curl or wget is required"
    fi

    tar xzf "${tmpdir}/archive.tar.gz" -C "${tmpdir}"

    if [ -w "${INSTALL_DIR}" ]; then
        mv "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    else
        printf "Installing to %s (requires sudo)\n" "${INSTALL_DIR}"
        sudo mv "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    fi

    chmod +x "${INSTALL_DIR}/${BINARY}"
    printf "Installed %s to %s\n" "${BINARY}" "${INSTALL_DIR}/${BINARY}"
}

get_latest_tag() {
    if command -v curl > /dev/null 2>&1; then
        curl -fsSL -o /dev/null -w '%{url_effective}' \
            "https://github.com/${REPO}/releases/latest" \
            | rev | cut -d'/' -f1 | rev
    elif command -v wget > /dev/null 2>&1; then
        wget -qO /dev/null --max-redirect=0 \
            "https://github.com/${REPO}/releases/latest" 2>&1 \
            | grep -i 'location' | sed 's|.*/||'
    else
        err "curl or wget is required"
    fi
}

err() {
    printf "Error: %s\n" "$1" >&2
    exit 1
}

main
