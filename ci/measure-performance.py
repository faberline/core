#!/usr/bin/env python3
"""Run the three measured performance workloads and check their thresholds.

Workloads (commands and thresholds kept from the removed `candidate measured
performance` job):
- http2: 128 reused-connection requests in strictly less than 5.0 seconds.
  The same run also prints requests per second, p50 and p99 latency.
- snapshot: 1000 small and 50000 large entries; the large peak must stay
  below max(small peak, 65536) * 4.
- raft_write: 500 sequential 128-byte proposals on a single-voter host. It
  prints requests per second, p50 and p99 latency.

The two latency workloads are compared with the stored baseline in
`ci/performance-baseline.json`. A workload fails when requests per second drops,
or p50 or p99 grows, by more than REGRESSION_THRESHOLD of the baseline.
"""

import argparse
import json
import math
import os
import re
import subprocess
import sys
from pathlib import Path

HTTP2_SUFFIX = '::generated_python_h2c_runtime_reused_connection_perf_guard'
SNAPSHOT_SUFFIX = 'snapshot_peak_memory_grows_sublinearly'
WRITE_SUFFIX = 'write_path_latency_is_measured'

# Fail when requests per second drops, or p50 or p99 grows, by more than this
# fraction of the baseline. This is the only place the number appears.
REGRESSION_THRESHOLD = 0.30
BASELINE_PATH = 'ci/performance-baseline.json'

HTTP2_COMMAND = [
    'cargo', 'test', '--release', '--locked', '-p', 'openapi-codegen', '--lib', '--all-features',
    '--', '--include-ignored', '--format', 'pretty', '--color', 'never', '--test-threads', '1',
    '--nocapture',
]
SNAPSHOT_COMMAND = [
    'cargo', 'test', '--release', '--locked', '-p', 'raft-runtime', '--test', 'snapshot_peak_memory',
    '--all-features', '--', '--include-ignored', '--format', 'pretty', '--color', 'never',
    '--test-threads', '1', '--nocapture',
]
WRITE_COMMAND = [
    'cargo', 'test', '--release', '--locked', '-p', 'raft-runtime', '--test', 'write_path_latency',
    '--all-features', '--', '--include-ignored', '--format', 'pretty', '--color', 'never',
    '--test-threads', '1', '--nocapture',
]
COMMANDS = {'http2': HTTP2_COMMAND, 'snapshot': SNAPSHOT_COMMAND, 'raft_write': WRITE_COMMAND}

RESULT_OK_RE = re.compile(r'^test result: ok\. \d+ passed; 0 failed; 0 ignored; 0 measured; 0 filtered out', re.M)
TEST_LINE_RE = re.compile(r'^test (\S+) \.\.\. (\w+)$', re.M)
H2_LINE_RE = re.compile(r'^h2c_generated_sync_reused_connection seconds=([0-9.]+) requests=(\d+)$', re.M)
H2_LATENCY_RE = re.compile(
    r'^h2c_generated_sync_latency requests=(\d+) rps=([0-9.]+) p50_ms=([0-9.]+) p99_ms=([0-9.]+)$',
    re.M,
)
WRITE_LATENCY_RE = re.compile(
    r'^CORE_WRITE_PATH_LATENCY requests=(\d+) rps=([0-9.]+) p50_ms=([0-9.]+) p99_ms=([0-9.]+)$',
    re.M,
)
SNAPSHOT_LINE_RE = re.compile(
    r'^CORE_SNAPSHOT_PEAK small_entries=(\d+) small_peak_bytes=(\d+) '
    r'large_entries=(\d+) large_peak_bytes=(\d+) limit_bytes=(\d+)$',
    re.M,
)


def check_roster(stdout, suffix, kind):
    """The required test must run, report `ok`, and the summary must be ok."""
    executed = any(
        name.endswith(suffix) and status == 'ok'
        for name, status in TEST_LINE_RE.findall(stdout)
    )
    if not executed or not RESULT_OK_RE.search(stdout):
        raise ValueError(f'required {kind} measurement test did not execute')


def check_http2(stdout: str, stderr: str) -> dict:
    check_roster(stdout, HTTP2_SUFFIX, 'http2')
    text = stdout + '\n' + stderr
    matches = H2_LINE_RE.findall(text)
    if len(matches) != 1:
        raise ValueError('missing or duplicate HTTP/2 measurement')
    seconds = float(matches[0][0])
    requests = int(matches[0][1])
    if not (math.isfinite(seconds) and 0 < seconds < 5.0 and requests == 128):
        raise ValueError('HTTP/2 original 128-request / 5-second threshold failed')
    return {
        'workload': 'http2',
        'measurement': {
            'seconds': seconds,
            'requests': requests,
            'seconds_exclusive_limit': 5.0,
        },
    }


def check_snapshot(stdout: str, stderr: str) -> dict:
    check_roster(stdout, SNAPSHOT_SUFFIX, 'snapshot')
    text = stdout + '\n' + stderr
    matches = SNAPSHOT_LINE_RE.findall(text)
    if len(matches) != 1:
        raise ValueError('missing or duplicate allocator measurement')
    small_entries, small_peak, large_entries, large_peak, limit = (int(v) for v in matches[0])
    if not (
        small_entries == 1000
        and large_entries == 50000
        and limit == max(small_peak, 65536) * 4
        and 0 <= large_peak < limit
    ):
        raise ValueError('snapshot original sublinear allocator threshold failed')
    return {
        'workload': 'snapshot',
        'measurement': {
            'small_entries': small_entries,
            'small_peak_bytes': small_peak,
            'large_entries': large_entries,
            'large_peak_bytes': large_peak,
            'bytes_exclusive_limit': limit,
        },
    }


def parse_latency(regex, text: str, label: str) -> dict:
    """Exactly one latency line, with finite positive numbers."""
    matches = regex.findall(text)
    if len(matches) != 1:
        raise ValueError(f'missing or duplicate {label} latency line')
    requests, rps, p50, p99 = matches[0]
    result = {
        'requests': int(requests),
        'requests_per_second': float(rps),
        'p50_ms': float(p50),
        'p99_ms': float(p99),
    }
    for field in ('requests_per_second', 'p50_ms', 'p99_ms'):
        value = result[field]
        if not (math.isfinite(value) and value > 0):
            raise ValueError(f'{label} latency {field} is not a positive number')
    return result


def check_http2_latency(stdout: str, stderr: str) -> dict:
    return parse_latency(H2_LATENCY_RE, stdout + '\n' + stderr, 'http2')


def check_write_path(stdout: str, stderr: str) -> dict:
    check_roster(stdout, WRITE_SUFFIX, 'raft write path')
    measurement = parse_latency(WRITE_LATENCY_RE, stdout + '\n' + stderr, 'raft write path')
    return {'workload': 'raft_write', 'measurement': measurement}


def check_regression(label, measured, baseline, threshold=REGRESSION_THRESHOLD):
    """Fail when the measured run is worse than the baseline by more than threshold.

    A run exactly at the limit passes.
    """
    rps_floor = baseline['requests_per_second'] * (1 - threshold)
    p50_ceiling = baseline['p50_ms'] * (1 + threshold)
    p99_ceiling = baseline['p99_ms'] * (1 + threshold)
    if measured['requests_per_second'] < rps_floor:
        raise ValueError(
            f"{label} regression: requests_per_second {measured['requests_per_second']} "
            f'is below {rps_floor:.3f}'
        )
    if measured['p50_ms'] > p50_ceiling:
        raise ValueError(
            f"{label} regression: p50_ms {measured['p50_ms']} is above {p50_ceiling:.3f}"
        )
    if measured['p99_ms'] > p99_ceiling:
        raise ValueError(
            f"{label} regression: p99_ms {measured['p99_ms']} is above {p99_ceiling:.3f}"
        )


def load_baseline(path) -> dict:
    path = Path(path)
    if not path.is_file():
        raise ValueError('missing performance baseline')
    data = json.loads(path.read_text())
    for kind in ('http2', 'raft_write'):
        if kind not in data:
            raise ValueError(f'performance baseline has no {kind} workload')
        for field in ('requests_per_second', 'p50_ms', 'p99_ms'):
            if field not in data[kind]:
                raise ValueError(f'performance baseline {kind} has no {field}')
    return data


def run_workload(kind: str, root: str) -> dict:
    command = COMMANDS[kind]
    completed = subprocess.run(command, cwd=root, capture_output=True, text=True)
    sys.stdout.write(completed.stdout)
    sys.stderr.write(completed.stderr)
    if completed.returncode != 0:
        raise ValueError(f'measured target failed with exit {completed.returncode}')
    if kind == 'http2':
        result = check_http2(completed.stdout, completed.stderr)
        result['measurement']['latency'] = check_http2_latency(completed.stdout, completed.stderr)
        return result
    if kind == 'raft_write':
        return check_write_path(completed.stdout, completed.stderr)
    return check_snapshot(completed.stdout, completed.stderr)


def git_commit(root: str) -> str:
    completed = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=root, capture_output=True, text=True)
    return completed.stdout.strip()


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path('.'))
    parser.add_argument('--baseline', type=Path, default=None)
    parser.add_argument('--write-baseline', action='store_true')
    args = parser.parse_args(argv)
    root = str(args.root)
    baseline_path = args.baseline if args.baseline is not None else Path(root) / BASELINE_PATH

    baseline = None
    baseline_error = None
    if not args.write_baseline:
        try:
            baseline = load_baseline(baseline_path)
        except ValueError as error:
            baseline_error = str(error)

    failed = False
    latencies = {}
    for kind in ('http2', 'snapshot', 'raft_write'):
        record = {
            'schema': 'CORE_PERFORMANCE_EVIDENCE_V1',
            'workload': kind,
            'commit': git_commit(root),
            'run_id': os.environ.get('GITHUB_RUN_ID'),
            'run_attempt': os.environ.get('GITHUB_RUN_ATTEMPT'),
        }
        try:
            result = run_workload(kind, root)
            measurement = result['measurement']
            if kind in ('http2', 'raft_write'):
                latency = measurement['latency'] if kind == 'http2' else measurement
                latencies[kind] = latency
                if not args.write_baseline:
                    if baseline_error is not None:
                        raise ValueError(baseline_error)
                    check_regression(kind, latency, baseline[kind])
            record['result'] = 'pass'
            record['measurement'] = measurement
        except ValueError as error:
            failed = True
            record['result'] = 'fail'
            record['error'] = str(error)
        print('CORE_PERFORMANCE_EVIDENCE_V1 ' + json.dumps(record, sort_keys=True))

    if args.write_baseline and not failed:
        new_baseline = {
            kind: {
                'requests_per_second': round(latencies[kind]['requests_per_second'], 3),
                'p50_ms': round(latencies[kind]['p50_ms'], 3),
                'p99_ms': round(latencies[kind]['p99_ms'], 3),
            }
            for kind in ('http2', 'raft_write')
        }
        baseline_path.parent.mkdir(parents=True, exist_ok=True)
        with open(baseline_path, 'w') as handle:
            json.dump(new_baseline, handle, indent=2, sort_keys=True)
            handle.write('\n')
        print(f'wrote {baseline_path}')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
