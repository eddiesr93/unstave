#!/usr/bin/env bash
set -euo pipefail

# Validates that a native binding artifact exists and is non-empty WITHOUT
# executing it.
#
# Musl-built NAPI addons are dynamically linked against musl libc and cannot
# be loaded (require()'d) on the glibc-based GitHub Actions runners. We cannot
# run the JS binding tests for musl targets in CI, so this is the maximum
# honest level of validation available: exercise the package at the artifact
# level (file exists and carries bytes).

dir="${1:?usage: validate-native-binding.sh <artifact-dir>}"

shopt -s nullglob
files=("$dir"/unstave.*.node)

if ((${#files[@]} == 0)); then
  echo "ERROR: no .node binding found in $dir" >&2
  exit 1
fi

for f in "${files[@]}"; do
  if [[ ! -s "$f" ]]; then
    echo "ERROR: binding is empty: $f" >&2
    exit 1
  fi
  echo "OK: $(basename "$f") ($(wc -c < "$f" | tr -d ' ') bytes)"
done
