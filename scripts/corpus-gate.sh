#!/usr/bin/env bash
#
# Runs the corpus-wide Go gate: every Python fixture is transpiled to Go, and
# every fixture the pipeline claims to support must survive `gofmt` and
# `go vet`.
#
# The interesting number is not how many fixtures transpile. It is how many the
# backend *claims* — emits with no error diagnostic — and then fails to compile.
# Those are the claims that are wrong, and they are the defect class the Go
# implementation shipped with. The classification is compared against
# `tests/corpus/go_gate_baseline.txt`, so this script fails when a claim that
# compiled stops compiling.
#
# Usage:
#
#   ./scripts/corpus-gate.sh              # verify against the baseline
#   ./scripts/corpus-gate.sh --update     # re-record the baseline
#
# Record a baseline update in the same commit as the capability that caused it;
# the diff in `tests/corpus/go_gate_baseline.txt` is the review evidence that
# coverage grew rather than the gate being loosened.

set -euo pipefail

cd "$(dirname "$0")/.."

if [ "${1:-}" = "--update" ]; then
    export UPDATE_CORPUS_GATE=1
fi

if ! command -v gofmt >/dev/null 2>&1 || ! command -v go >/dev/null 2>&1; then
    echo "warning: the Go toolchain is not on PATH; the classification is printed but not compared" >&2
fi

exec cargo test --test corpus_go_gate -- --nocapture