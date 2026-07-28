#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 2 ]]; then
    echo "usage: package.sh <app-path> <output.dmg>" >&2
    exit 2
fi

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_dir="$(cd "${script_dir}/../.." && pwd)"
app_path="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
output_dir="$(cd "$(dirname "$2")" && pwd)"
output_path="${output_dir}/$(basename "$2")"
icon_icns="${repo_dir}/assets/icon.icns"

if [[ ! -d "${app_path}" ]]; then
    echo "application bundle not found: ${app_path}" >&2
    exit 1
fi
if [[ ! -f "${icon_icns}" ]]; then
    echo "volume icon not found: ${icon_icns}" >&2
    exit 1
fi
if ! command -v dmgbuild >/dev/null 2>&1; then
    echo "dmgbuild is required" >&2
    exit 1
fi

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/shrieker-dmg.XXXXXX")"
background_path="${work_dir}/background.tiff"

cleanup() {
    rm -rf "${work_dir}"
}
trap cleanup EXIT

CLANG_MODULE_CACHE_PATH="${work_dir}/clang-cache" \
SWIFT_MODULECACHE_PATH="${work_dir}/swift-cache" \
swift "${script_dir}/background.swift" \
    "${background_path}"

rm -f "${output_path}"
dmgbuild \
    -s "${script_dir}/dmg.py" \
    -D "app=${app_path}" \
    -D "icon=${icon_icns}" \
    -D "background=${background_path}" \
    "Shrieker" \
    "${output_path}"

echo "created ${output_path}"
