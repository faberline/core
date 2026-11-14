# Performance

This page holds the measured performance numbers of core, the rule that a
release is checked against them, and how to refresh them. The check runs in the
release workflow, job `perf: thresholds`, with
`python3 ci/measure-performance.py --root .`.

## What is measured

- **http2**: 128 reused-connection requests from the generated Python client to
  the h2c test server. The test `generated_python_h2c_runtime_reused_connection_perf_guard`
  (`crates/openapi-codegen/src/domain/emit/py/tests/h2c_connection.rs`) prints the
  requests per second and the p50 and p99 latency of each request.
- **raft write path**: 500 sequential 128-byte proposals on a single-voter host
  with `FsyncPolicy::Always`, after 50 untimed warm-up proposals. The test
  `write_path_latency_is_measured`
  (`crates/raft-runtime/tests/write_path_latency.rs`) prints the requests per
  second and the p50 and p99 latency.
- **snapshot peak memory**: 1000 small and 50000 large entries, checked against
  the test `snapshot_peak_memory_grows_sublinearly`
  (`crates/raft-runtime/tests/snapshot_peak_memory.rs`). This workload has no
  baseline; it is checked only against the limit below.

## Numbers

The baseline is in [`ci/performance-baseline.json`](../../ci/performance-baseline.json).
It was measured on Linux x86_64 with 4 CPUs on 2026-11-14, at commit `438c561f`.

| Workload | Requests per second | p50 ms | p99 ms |
|---|---|---|---|
| http2, reused connection (128 requests) | 4261.578 | 0.206 | 0.371 |
| raft write path (500 proposals) | 1481.328 | 0.639 | 1.151 |

The numbers depend on the machine. Compare runs only from one class of runner.

The snapshot test passed on that run, but both of its peak values were `0`
bytes. Its check therefore proves little on that machine, and the page makes no
memory number claim for it.

## Limits that never change

These limits are in `ci/measure-performance.py` and are not changed by the
baseline:

- http2: 128 requests in strictly less than 5.0 seconds.
- snapshot: the large-entry peak must be below `max(small peak, 65536) * 4`.

## Regression check

The http2 and raft write paths fail when requests per second drops, or p50 or p99
latency grows, by more than 30 percent of the baseline. The limit is the one
named constant `REGRESSION_THRESHOLD` in `ci/measure-performance.py`. A run that
is exactly at the limit passes.

## Refresh the baseline

Refresh the baseline only after a runner change or an accepted slowdown. Make the
change in a pull request that says why:

1. Run `python3 ci/measure-performance.py --root . --write-baseline`. It runs the
   workloads and writes `ci/performance-baseline.json` only when every workload
   passes.
2. Commit `ci/performance-baseline.json` and update the table on this page with
   the new numbers, the machine and the date.

Never change `REGRESSION_THRESHOLD` or the limits above to make a run pass.
