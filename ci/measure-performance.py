#!/usr/bin/env python3
"""Run the two measured performance workloads and check their original thresholds.

Workloads (commands and thresholds kept from the removed `candidate measured
performance` job):
- http2: 128 reused-connection requests in strictly less than 5.0 seconds.
- snapshot: 1000 small and 50000 large entries; the large peak must stay
  below max(small peak, 65536) * 4.
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

RESULT_OK_RE = re.compile(r'^test result: ok\. \d+ passed; 0 failed; 0 ignored; 0 measured; 0 filtered out', re.M)
TEST_LINE_RE = re.compile(r'^test (\S+) \.\.\. (\w+)$', re.M)
H2_LINE_RE = re.compile(r'^h2c_generated_sync_reused_connection seconds=([0-9.]+) requests=(\d+)$', re.M)
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


def run_workload(kind: str, root: str) -> dict:
    command = HTTP2_COMMAND if kind == 'http2' else SNAPSHOT_COMMAND
    completed = subprocess.run(command, cwd=root, capture_output=True, text=True)
    sys.stdout.write(completed.stdout)
    sys.stderr.write(completed.stderr)
    if completed.returncode != 0:
        raise ValueError(f'measured target failed with exit {completed.returncode}')
    if kind == 'http2':
        return check_http2(completed.stdout, completed.stderr)
    return check_snapshot(completed.stdout, completed.stderr)


def git_commit(root: str) -> str:
    completed = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=root, capture_output=True, text=True)
    return completed.stdout.strip()


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path('.'))
    args = parser.parse_args(argv)
    root = str(args.root)

    failed = False
    for kind in ('http2', 'snapshot'):
        record = {
            'schema': 'CORE_PERFORMANCE_EVIDENCE_V1',
            'workload': kind,
            'commit': git_commit(root),
            'run_id': os.environ.get('GITHUB_RUN_ID'),
            'run_attempt': os.environ.get('GITHUB_RUN_ATTEMPT'),
        }
        try:
            result = run_workload(kind, root)
            record['result'] = 'pass'
            record['measurement'] = result['measurement']
        except ValueError as error:
            failed = True
            record['result'] = 'fail'
            record['error'] = str(error)
        print('CORE_PERFORMANCE_EVIDENCE_V1 ' + json.dumps(record, sort_keys=True))
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
