#!/usr/bin/env bash
#
# Measures CLI startup latency for a `gset` binary.
#
# Startup is the dominant cost of a per-invocation transpiler, so it is budgeted
# against the Go baseline recorded in docs/RUST_REWRITE_PLAN.md. This script
# reproduces that measurement's shape: N invocations of `gset version`,
# reporting the median, p90 and minimum wall time in milliseconds.
#
# Usage:
#
#   cargo build --release -p gset-cli
#   ./scripts/measure-startup.sh target/release/gset [count]
#
# Compare binaries by running each in turn; do not interleave by hand, and keep
# the machine otherwise idle. Timing is done by a Python harness, not by `date`,
# because spawning `date` twice per sample costs more than the thing being
# measured.

set -euo pipefail

BIN="${1:-target/release/gset}"
COUNT="${2:-200}"

if [ ! -x "$BIN" ]; then
    echo "error: no executable at $BIN" >&2
    exit 1
fi

python3 - "$BIN" "$COUNT" <<'PY'
import statistics
import subprocess
import sys
import time

binary, count = sys.argv[1], int(sys.argv[2])
devnull = subprocess.DEVNULL

# A warm-up pass keeps first-run page-cache misses out of the sample.
for _ in range(10):
    subprocess.run([binary, "version"], stdout=devnull, stderr=devnull)

samples = []
for _ in range(count):
    start = time.perf_counter()
    subprocess.run([binary, "version"], stdout=devnull, stderr=devnull)
    samples.append((time.perf_counter() - start) * 1000.0)

samples.sort()


def percentile(fraction):
    index = min(len(samples) - 1, int(len(samples) * fraction))
    return samples[index]


print(
    "n={}  median={:.2f} ms  p90={:.2f} ms  min={:.2f} ms".format(
        count,
        statistics.median(samples),
        percentile(0.90),
        samples[0],
    )
)
PY
