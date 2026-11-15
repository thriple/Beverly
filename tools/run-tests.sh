#!/usr/bin/env bash
# Run every scripted test (see editor/src/scripted.rs) in every test project under tests/, or in the projects given.
#   tools/run-tests.sh [PLAYER] [PROJECT...]     PLAYER defaults to the debug beverly-player
# Each project is a normal Beverly project with test files in its tests/ folder. Exits 1 if any test failed.
set -o pipefail
cd "$(dirname "$0")/.."
player=${1:-${CARGO_TARGET_DIR:-editor/target}/debug/beverly-player}
shift
projects=("$@"); [ ${#projects[@]} -eq 0 ] && projects=(tests/*/)
failed=0
for p in "${projects[@]}"; do
  for t in "$p"/tests/*.ron; do
    [ -e "$t" ] || continue
    "$player" "$p" --test "tests/$(basename "$t")" 2>/dev/null | tail -1 || failed=1
  done
done
exit $failed
