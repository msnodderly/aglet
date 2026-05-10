#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C
export LANG=C

usage() {
  cat <<'USAGE'
Usage: scripts/build-standalone-aglet.sh [--out-dir DIR]

Builds the release-mode aglet binary and packages it for distribution.
The output directory receives:
  - aglet-<version>-<target>.tar.gz
  - aglet-<version>-<target>.tar.gz.sha256
USAGE
}

out_dir="dist"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out-dir)
      if [[ $# -lt 2 ]]; then
        echo "error: --out-dir requires a directory" >&2
        exit 2
      fi
      out_dir="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$(uname -s)" in
  Darwin) os="macos" ;;
  Linux) os="linux" ;;
  MINGW*|MSYS*|CYGWIN*) os="windows" ;;
  *)
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    ;;
esac

case "$(uname -m)" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *)
    arch="$(uname -m | tr '[:upper:]' '[:lower:]')"
    ;;
esac

target_label="${os}-${arch}"
version="$(cargo metadata --format-version 1 --no-deps \
  | sed -n 's/.*"name":"aglet-cli","version":"\([^"]*\)".*/\1/p')"

if [[ -z "$version" ]]; then
  echo "error: could not determine aglet-cli version from cargo metadata" >&2
  exit 1
fi

cargo build --release --locked --bin aglet

binary="target/release/aglet"
binary_name="aglet"
if [[ "$os" == "windows" ]]; then
  binary="target/release/aglet.exe"
  binary_name="aglet.exe"
fi

if [[ ! -x "$binary" ]]; then
  echo "error: expected binary was not built: $binary" >&2
  exit 1
fi

package_stem="aglet-${version}-${target_label}"
package_dir="${out_dir}/${package_stem}"
archive="${out_dir}/${package_stem}.tar.gz"

rm -rf "$package_dir" "$archive" "${archive}.sha256"
mkdir -p "$package_dir"

cp "$binary" "${package_dir}/${binary_name}"
chmod +x "${package_dir}/${binary_name}"

cat > "${package_dir}/README.txt" <<EOF
aglet ${version} (${target_label})

Run ./aglet --help to see available commands.
Run ./aglet without a subcommand to open the TUI.
EOF

tar -C "$out_dir" -czf "$archive" "$package_stem"

if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$archive" > "${archive}.sha256"
elif command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$archive" > "${archive}.sha256"
else
  echo "warning: neither shasum nor sha256sum is available; skipping checksum" >&2
fi

echo "built $archive"
