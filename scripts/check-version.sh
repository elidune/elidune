#!/usr/bin/env bash
# Fail if server/Cargo.toml and ui/package.json versions differ.
# With --tag <ref>, also require that they match the git tag (v prefix optional).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  cat <<'EOF'
Usage: check-version.sh [--tag <vX.Y.Z>]

Compare server/Cargo.toml and ui/package.json versions.
When --tag is set (e.g. v1.3.11 or 1.3.11), also require a match with that tag.
EOF
}

read_cargo_version() {
  awk '
    /^\[package\]/ { in_pkg = 1; next }
    /^\[/ { in_pkg = 0 }
    in_pkg && /^version[[:space:]]*=/ {
      gsub(/[" ]/, "", $3)
      print $3
      exit
    }
  ' "${ROOT}/server/Cargo.toml"
}

read_ui_version() {
  python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' \
    "${ROOT}/ui/package.json"
}

TAG_REF=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag)
      TAG_REF="${2:?--tag requires a value}"
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

CARGO_VERSION="$(read_cargo_version)"
UI_VERSION="$(read_ui_version)"

if [[ -z "${CARGO_VERSION}" || -z "${UI_VERSION}" ]]; then
  echo "error: could not read versions (cargo='${CARGO_VERSION}' ui='${UI_VERSION}')" >&2
  exit 1
fi

if [[ "${CARGO_VERSION}" != "${UI_VERSION}" ]]; then
  echo "error: version mismatch: server/Cargo.toml=${CARGO_VERSION} ui/package.json=${UI_VERSION}" >&2
  exit 1
fi

if [[ -n "${TAG_REF}" ]]; then
  TAG_VERSION="${TAG_REF#v}"
  if [[ "${CARGO_VERSION}" != "${TAG_VERSION}" ]]; then
    echo "error: git tag ${TAG_REF} does not match manifests (${CARGO_VERSION})" >&2
    exit 1
  fi
fi

echo "version ${CARGO_VERSION} (server = ui${TAG_REF:+ = ${TAG_REF}})"
