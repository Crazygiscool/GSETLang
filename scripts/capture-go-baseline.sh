#!/usr/bin/env bash
#
# Captures the Go implementation's output for every bug-class input.
#
# These outputs are NOT the target of the rewrite. They are recorded because the
# rewrite must *avoid* them, and a defect that is only described in prose is a
# defect that quietly comes back. See tests/baseline/README.md.
#
# Run before milestone 6 deletes the Go tree:
#
#   ./scripts/capture-go-baseline.sh
#
# Requires a `gset` binary built from the Go sources:
#
#   go build -o gset . && ./scripts/capture-go-baseline.sh
#
# Review the diff before committing. A change here means the Go implementation
# changed, which should not happen after the fixtures were first recorded.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${GSET_BIN:-$ROOT/gset}"
INPUTS="$ROOT/tests/baseline/inputs"
OUTPUTS="$ROOT/tests/baseline/outputs"

if [ ! -x "$BIN" ]; then
    echo "error: no gset binary at $BIN" >&2
    echo "build one with: go build -o gset ." >&2
    exit 1
fi

# Target name -> output extension. Kept in the same order as the CI syntax-check
# matrix so a missing target is obvious.
TARGETS=(go python javascript java ruby)

rm -rf "$OUTPUTS"
mkdir -p "$OUTPUTS"

for input in "$INPUTS"/*.gset; do
    class="$(basename "$input" .gset)"
    mkdir -p "$OUTPUTS/$class"

    for target in "${TARGETS[@]}"; do
        dest="$OUTPUTS/$class/gset.$target"

        # -o writes the transpiled source directly, bypassing the log line that
        # transpile otherwise interleaves on some targets.
        if (cd "$ROOT" && "$BIN" transpile "$input" --target "$target" -o "$dest") \
            >/dev/null 2>"$dest.stderr"; then
            :
        else
            # A non-zero exit means the Go implementation could not handle the
            # input at all. Record the failure rather than an empty file, so the
            # fixture still documents the behaviour.
            {
                echo "# transpile failed with a non-zero exit status"
                cat "$dest.stderr"
            } >"$dest"
        fi

        # Defence in depth: strip any log line that leaked into the output file.
        sed -i -E '/^\[[0-9]{4}-[0-9]{2}-[0-9]{2} /d' "$dest" 2>/dev/null || true
        rm -f "$dest.stderr"
    done

    printf 'captured %s\n' "$class"
done

echo
echo "Wrote $(find "$OUTPUTS" -type f | wc -l) files to $OUTPUTS"
