# Performance

This page holds the measured performance numbers of core, the rule that a release must meet, and how to refresh the stored baseline. The check runs in the release job `perf-thresholds` of `.github/workflows/release.yml`, with `python3 ci/measure-performance.py --root .`.

## What is measured

- **http2**: 128 requests on one reused HTTP/2 connection (cleartext), made by the Python client that `openapi-codegen` generates, against a test server.
- **raft_write**: 500 sequential proposals of 128 bytes each to a single-voter host with `FsyncPolicy::Always`, after 50 untimed warm-up proposals. The test is `crates/raft-runtime/tests/write_path_latency.rs`.
- **snapshot**: the peak memory of a snapshot of 1000 small and 50000 large entries. It has no throughput or latency numbers.

For the two latency workloads the script records requests per second and the p50 and p99 latency of each request.

## Numbers

| Workload | Requests per second | p50 ms | p99 ms |
|---|---|---|---|
| http2 (128 requests, one connection) | 5419.539 | 0.135 | 0.276 |
| raft_write (500 proposals, one voter) | 1420.024 | 0.671 | 1.277 |

Measured on Linux x86_64 with 4 CPUs, on 2026-11-14 (UTC). The numbers depend on the machine and its load, so compare runs only with runs from the same runner class. The stored values are in `ci/performance-baseline.json`. The release job uses the label `ubuntu-latest`. The image behind it changed from ubuntu-24.04 to ubuntu-26.04 between v0.5.0 and v0.5.1-rc.1, and the stored numbers were not measured on a release runner (issue #455).

## Limits that never change

The original limits are not part of the baseline, and a baseline refresh never changes them:

- http2: the 128 requests take strictly less than 5.0 seconds.
- snapshot: the large peak stays below `max(small peak, 65536) * 4`.

They are set in `ci/measure-performance.py`.

## Regression check

A run measures each latency workload 5 times (`REPEATS` in `ci/measure-performance.py`, or `--repeats`) and uses the median of each metric. It fails when, compared with the baseline:

- requests per second drops by more than 30 percent of the baseline, or
- p50 or p99 latency grows by more than 30 percent of the baseline.

The number is the constant `REGRESSION_THRESHOLD` in `ci/measure-performance.py`. A result exactly at the limit passes. The evidence line `CORE_PERFORMANCE_EVIDENCE_V1` of each workload lists every sample under `samples`, so a refresh of the baseline can use the real numbers of the release runner.

## Refresh the baseline

Refresh the baseline only after a change of the runner, or after a slowdown that the team has accepted. Do it in a pull request that says why.

1. Run `python3 ci/measure-performance.py --root . --write-baseline`. It writes `ci/performance-baseline.json` when all workloads pass.
2. Run `python3 ci/measure-performance.py --root .` again and check that it passes.
3. Commit `ci/performance-baseline.json` and update the table above with the same numbers.

Never change the threshold or a limit to make a run pass.
