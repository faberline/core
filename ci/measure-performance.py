#!/usr/bin/env python3
"""Run the three measured performance workloads and check their thresholds.

Workloads (commands and thresholds kept from the removed `candidate measured
performance` job):
- http2: 128 reused-connection requests in strictly less than 5.0 seconds.
  Its requests per second and p50 and p99 latency are compared with the
  baseline.
- snapshot: 1000 small and 50000 large entries; the large peak must stay
  below max(small peak, 65536) * 4.
- raft_write: 500 sequential single-voter proposals. Its requests per second
  and p50 and p99 latency are compared with the baseline.

A result is a regression when requests per second drops, or p50 or p99 grows,
by more than REGRESSION_THRESHOLD of the baseline value in
ci/performance-baseline.json. Refresh that file with --write-baseline.
The http2 and raft_write workloads run REPEATS times and the median of each metric is compared with the baseline; the evidence line lists every sample.
"""

import argparse
import json
import math
import os
import re
import statistics
import subprocess
import sys
from pathlib import Path

HTTP2_SUFFIX = '::generated_python_h2c_runtime_reused_connection_perf_guard'
SNAPSHOT_SUFFIX = 'snapshot_peak_memory_grows_sublinearly'
WRITE_SUFFIX = 'write_path_latency_is_measured'

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

# Fail when requests per second drops, or p50 or p99 grows, by more than this
# fraction of the baseline. The only place this number is set.
REGRESSION_THRESHOLD = 0.30
# Each compared workload is measured this many times; the gate uses the median. Odd, so the median is a real sample. The only place this number is set.
REPEATS = 5
BASELINE_PATH = 'ci/performance-baseline.json'
COMPARED = ('http2', 'raft_write')
LATENCY_FIELDS = ('requests_per_second', 'p50_ms', 'p99_ms')

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


def parse_latency(regex, text, label) -> dict:
    """Read one latency line: requests, requests per second, p50 and p99."""
    matches = regex.findall(text)
    if len(matches) != 1:
        raise ValueError(f'missing or duplicate {label} latency line')
    requests, rps, p50, p99 = matches[0]
    values = {
        'requests_per_second': float(rps),
        'p50_ms': float(p50),
        'p99_ms': float(p99),
    }
    for field, value in values.items():
        if not (math.isfinite(value) and value > 0):
            raise ValueError(f'{label} latency {field} must be finite and greater than 0')
    return {'requests': int(requests), **values}


def check_http2_latency(stdout: str, stderr: str) -> dict:
    return parse_latency(H2_LATENCY_RE, stdout + '\n' + stderr, 'http2')


def check_write_path(stdout: str, stderr: str) -> dict:
    check_roster(stdout, WRITE_SUFFIX, 'raft write path')
    measurement = parse_latency(WRITE_LATENCY_RE, stdout + '\n' + stderr, 'raft write path')
    return {'workload': 'raft_write', 'measurement': measurement}


def check_regression(label, measured, baseline, threshold=REGRESSION_THRESHOLD):
    """Raise ValueError when a result is more than `threshold` worse than the baseline."""
    if measured['requests_per_second'] < baseline['requests_per_second'] * (1 - threshold):
        raise ValueError(
            f'{label} regression: requests_per_second {measured["requests_per_second"]} '
            f'is more than {threshold:.0%} below baseline {baseline["requests_per_second"]}'
        )
    for field in ('p50_ms', 'p99_ms'):
        if measured[field] > baseline[field] * (1 + threshold):
            raise ValueError(
                f'{label} regression: {field} {measured[field]} '
                f'is more than {threshold:.0%} above baseline {baseline[field]}'
            )
    return None


def load_baseline(path) -> dict:
    path = Path(path)
    if not path.exists():
        raise ValueError('missing performance baseline')
    data = json.loads(path.read_text())
    for kind in COMPARED:
        if kind not in data:
            raise ValueError(f'performance baseline lacks {kind}')
        for field in LATENCY_FIELDS:
            if field not in data[kind]:
                raise ValueError(f'performance baseline lacks {kind}.{field}')
    return data


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


def run_workload(kind: str, root: str) -> dict:
    completed = subprocess.run(COMMANDS[kind], cwd=root, capture_output=True, text=True)
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


def median_latency(samples: list) -> dict:
    """The median of each latency field over several samples."""
    if not samples:
        raise ValueError('no latency samples')
    result = {'requests': samples[0]['requests']}
    for field in LATENCY_FIELDS:
        result[field] = round(statistics.median(s[field] for s in samples), 3)
    return result


def run_repeated(kind: str, root: str, repeats: int) -> dict:
    """Run a compared workload `repeats` times; the result carries the median of each metric."""
    results = [run_workload(kind, root) for _ in range(repeats)]
    if kind == 'raft_write':
        samples = [r['measurement'] for r in results]
        median = median_latency(samples)
        return {'workload': 'raft_write', 'measurement': {**median, 'repeats': repeats, 'samples': samples}}
    samples = [r['measurement']['latency'] for r in results]
    worst = max(results, key=lambda r: r['measurement']['seconds'])
    measurement = dict(worst['measurement'])
    measurement['latency'] = median_latency(samples)
    measurement['repeats'] = repeats
    measurement['samples'] = samples
    return {'workload': 'http2', 'measurement': measurement}


def git_commit(root: str) -> str:
    completed = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=root, capture_output=True, text=True)
    return completed.stdout.strip()


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path('.'))
    parser.add_argument('--baseline', type=Path, default=None)
    parser.add_argument('--write-baseline', action='store_true')
    parser.add_argument('--repeats', type=int, default=REPEATS)
    args = parser.parse_args(argv)
    if args.repeats < 1:
        parser.error('--repeats must be 1 or more')
    root = str(args.root)
    baseline_path = args.baseline if args.baseline is not None else Path(root) / BASELINE_PATH

    baseline = None
    baseline_error = None
    if not args.write_baseline:
        try:
            baseline = load_baseline(baseline_path)
        except ValueError as error:
            baseline_error = error

    failed = False
    measured = {}
    for kind in ('http2', 'snapshot', 'raft_write'):
        record = {
            'schema': 'CORE_PERFORMANCE_EVIDENCE_V1',
            'workload': kind,
            'commit': git_commit(root),
            'run_id': os.environ.get('GITHUB_RUN_ID'),
            'run_attempt': os.environ.get('GITHUB_RUN_ATTEMPT'),
        }
        try:
            result = run_repeated(kind, root, args.repeats) if kind in COMPARED else run_workload(kind, root)
            if kind in COMPARED:
                latency = result['measurement']['latency'] if kind == 'http2' else result['measurement']
                measured[kind] = latency
                if not args.write_baseline:
                    if baseline_error is not None:
                        raise baseline_error
                    check_regression(kind, latency, baseline[kind])
            record['result'] = 'pass'
            record['measurement'] = result['measurement']
        except ValueError as error:
            failed = True
            record['result'] = 'fail'
            record['error'] = str(error)
        print('CORE_PERFORMANCE_EVIDENCE_V1 ' + json.dumps(record, sort_keys=True))

    if args.write_baseline and not failed:
        data = {
            kind: {field: round(measured[kind][field], 3) for field in LATENCY_FIELDS}
            for kind in COMPARED
        }
        with open(baseline_path, 'w') as handle:
            json.dump(data, handle, indent=2, sort_keys=True)
            handle.write('\n')
        print(f'wrote {baseline_path}')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
