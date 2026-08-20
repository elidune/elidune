#!/usr/bin/env bash
# Bump the unique Elidune product version in server/Cargo.toml and ui/package.json.
# Optionally create a commit and annotated tag vX.Y.Z.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK_VERSION="${ROOT}/scripts/check-version.sh"

usage() {
  cat <<'EOF'
Usage: bump-version.sh <major|minor|patch|X.Y.Z[-prerelease]> [--tag]

Updates:
  server/Cargo.toml
  ui/package.json
  server/Cargo.lock  (local only; gitignored)

--tag  commit the bump and create annotated tag vX.Y.Z (does not push)

Examples:
  ./scripts/bump-version.sh patch
  ./scripts/bump-version.sh 1.4.0 --tag
EOF
}

is_semver() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]
}

read_current() {
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

bump_part() {
  local current="$1" part="$2"
  local core="${current%%-*}"
  local major minor patch
  IFS=. read -r major minor patch <<<"${core}"
  case "${part}" in
    major) echo "$((major + 1)).0.0" ;;
    minor) echo "${major}.$((minor + 1)).0" ;;
    patch) echo "${major}.${minor}.$((patch + 1))" ;;
    *)
      echo "error: unknown bump ${part}" >&2
      exit 2
      ;;
  esac
}

write_versions() {
  local version="$1"
  python3 - "${ROOT}" "${version}" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
version = sys.argv[2]

cargo_toml = root / "server" / "Cargo.toml"
text = cargo_toml.read_text()
in_pkg = False
out = []
replaced = False
for line in text.splitlines(keepends=True):
    stripped = line.lstrip()
    if stripped.startswith("[package]"):
        in_pkg = True
    elif stripped.startswith("["):
        in_pkg = False
    if in_pkg and re.match(r"^version\s*=", line) and not replaced:
        nl = "\n" if line.endswith("\n") else ""
        out.append(f'version = "{version}"{nl}')
        replaced = True
        continue
    out.append(line)
if not replaced:
    sys.exit("error: version field not found in server/Cargo.toml [package]")
cargo_toml.write_text("".join(out))

cargo_lock = root / "server" / "Cargo.lock"
if cargo_lock.exists():
    lock = cargo_lock.read_text()
    lock, n = re.subn(
        r'(name = "elidune-server"\nversion = ")[^"]+(")',
        rf"\g<1>{version}\g<2>",
        lock,
        count=1,
    )
    if n == 1:
        cargo_lock.write_text(lock)

pkg = root / "ui" / "package.json"
pkg_text = pkg.read_text()
pkg_text, n = re.subn(
    r'("version"\s*:\s*")[^"]*(")',
    rf"\g<1>{version}\g<2>",
    pkg_text,
    count=1,
)
if n != 1:
    sys.exit("error: version field not found in ui/package.json")
pkg.write_text(pkg_text)
PY
}

DO_TAG=0
SPEC=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag)
      DO_TAG=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ -n "${SPEC}" ]]; then
        echo "error: unexpected argument: $1" >&2
        usage >&2
        exit 2
      fi
      SPEC="$1"
      shift
      ;;
  esac
done

if [[ -z "${SPEC}" ]]; then
  usage >&2
  exit 2
fi

CURRENT="$(read_current)"
case "${SPEC}" in
  major|minor|patch)
    NEW_VERSION="$(bump_part "${CURRENT}" "${SPEC}")"
    ;;
  *)
    if ! is_semver "${SPEC}"; then
      echo "error: invalid version '${SPEC}' (expected X.Y.Z or major|minor|patch)" >&2
      exit 2
    fi
    NEW_VERSION="${SPEC}"
    ;;
esac

if [[ "${NEW_VERSION}" == "${CURRENT}" ]]; then
  echo "error: already at ${CURRENT}" >&2
  exit 1
fi

if [[ "${DO_TAG}" -eq 1 ]]; then
  if [[ -n "$(git -C "${ROOT}" status --porcelain)" ]]; then
    echo "error: working tree is not clean; commit or stash before --tag" >&2
    exit 1
  fi
  if git -C "${ROOT}" rev-parse -q --verify "refs/tags/v${NEW_VERSION}" >/dev/null; then
    echo "error: tag v${NEW_VERSION} already exists" >&2
    exit 1
  fi
fi

write_versions "${NEW_VERSION}"
"${CHECK_VERSION}"

echo "bumped ${CURRENT} -> ${NEW_VERSION}"

if [[ "${DO_TAG}" -eq 1 ]]; then
  git -C "${ROOT}" add \
    server/Cargo.toml \
    ui/package.json
  git -C "${ROOT}" commit -m "Release v${NEW_VERSION}"
  git -C "${ROOT}" tag -a "v${NEW_VERSION}" -m "v${NEW_VERSION}"
  echo "created tag v${NEW_VERSION}"
  echo "next: git push origin HEAD && git push origin v${NEW_VERSION}"
fi
