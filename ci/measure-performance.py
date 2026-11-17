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

With `--base-root` the run compares the head tree with the base tree measured on the same machine (relative gate); the baseline file is a reference only.
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


def check_relative(label, head, base, threshold=REGRESSION_THRESHOLD):
    """Raise ValueError when head is more than `threshold` worse than base, measured on the same machine."""
    if head['requests_per_second'] < base['requests_per_second'] * (1 - threshold):
        raise ValueError(
            f'{label} regression: requests_per_second {head["requests_per_second"]} '
            f'is more than {threshold:.0%} below base {base["requests_per_second"]}'
        )
    for field in ('p50_ms', 'p99_ms'):
        if head[field] > base[field] * (1 + threshold):
            raise ValueError(
                f'{label} regression: {field} {head[field]} '
                f'is more than {threshold:.0%} above base {base[field]}'
            )
    return None


def median_or_none(samples: list):
    return median_latency(samples) if samples else None


def measure_ab(base_root: str, head_root: str, repeats: int):
    """Measure the base and the head tree on this machine, base then head for each repeat.

    Returns (base_samples, head_samples, errors); each samples value is {kind: [latency dict, ...]}.
    """
    roots = (('base', base_root), ('head', head_root))
    for _, root in roots:
        for kind in COMPARED:
            cmd = list(COMMANDS[kind])
            cmd.insert(cmd.index('--'), '--no-run')
            subprocess.run(cmd, cwd=root, check=True)

    samples = {label: {kind: [] for kind in COMPARED} for label, _ in roots}
    errors = []
    for i in range(repeats):
        for kind in COMPARED:
            for label, root in roots:
                try:
                    result = run_workload(kind, root)
                except ValueError as error:
                    errors.append(f'{label} {kind} run {i + 1}: {error}')
                    continue
                value = result['measurement']['latency'] if kind == 'http2' else result['measurement']
                samples[label][kind].append(value)
    return samples['base'], samples['head'], errors


def ab_report(base_ref, head_ref, base_sha, head_sha, repeats, base_samples, head_samples,
              errors, baseline, limit):
    """Build the report dict and its markdown table from the A/B samples."""
    samples = {'base': base_samples, 'head': head_samples}
    medians = {
        'base': {kind: median_or_none(base_samples[kind]) for kind in COMPARED},
        'head': {kind: median_or_none(head_samples[kind]) for kind in COMPARED},
    }
    ratios = {}
    for kind in COMPARED:
        base_med, head_med = medians['base'][kind], medians['head'][kind]
        if base_med is None or head_med is None:
            ratios[kind] = None
        else:
            ratios[kind] = {
                field: round(head_med[field] / base_med[field], 3) for field in LATENCY_FIELDS
            }
    report = {
        'base_ref': base_ref,
        'head_ref': head_ref,
        'base_sha': base_sha,
        'head_sha': head_sha,
        'repeats': repeats,
        'limit': limit,
        'runner_image': os.environ.get('ImageVersion'),
        'run_id': os.environ.get('GITHUB_RUN_ID'),
        'samples': samples,
        'medians': medians,
        'baseline': None if baseline is None else {kind: baseline[kind] for kind in COMPARED},
        'ratios': ratios,
        'errors': errors,
    }

    lines = [f'## perf: base `{base_ref}` at `{base_sha}`, head `{head_ref}` at `{head_sha}`', '']
    if limit is None:
        lines.append('Report only: no limit is applied.')
    else:
        lines.append(
            f'Limit: {limit:.0%} worse than base (REGRESSION_THRESHOLD). '
            'The baseline file is shown as a reference only.'
        )
    lines.append('')
    for kind in COMPARED:
        lines += [
            f'### {kind}', '',
            '| ref | p50_ms runs | p50_ms median | p99_ms median | requests_per_second median |',
            '|---|---|---|---|---|',
        ]
        for label in ('base', 'head'):
            values = samples[label][kind]
            med = medians[label][kind]
            runs = ', '.join(str(v['p50_ms']) for v in values) or 'none'
            if med is None:
                lines.append(f'| {label} | {runs} | - | - | - |')
            else:
                lines.append(
                    f'| {label} | {runs} | {med["p50_ms"]} | {med["p99_ms"]} | {med["requests_per_second"]} |'
                )
        if baseline is not None:
            b = baseline[kind]
            lines.append(
                f'| baseline | - | {b["p50_ms"]} | {b["p99_ms"]} | {b["requests_per_second"]} |'
            )
        ratio = ratios[kind]
        if ratio is not None:
            lines.append('')
            lines.append(
                f'ratio head / base: rps {ratio["requests_per_second"]}, '
                f'p50 {ratio["p50_ms"]}, p99 {ratio["p99_ms"]}'
            )
        lines.append('')
    if errors:
        lines += ['### errors', ''] + [f'- {e}' for e in errors] + ['']
    return report, '\n'.join(lines) + '\n'


def run_ab(args, root: str) -> int:
    """The base and head trees on one machine, compared by check_relative (or reported only)."""
    base_root = str(args.base_root)
    base_samples, head_samples, errors = measure_ab(base_root, root, args.repeats)
    medians = {
        'base': {kind: median_or_none(base_samples[kind]) for kind in COMPARED},
        'head': {kind: median_or_none(head_samples[kind]) for kind in COMPARED},
    }
    failed = False
    missing = [
        f'{label} {kind}'
        for label in ('base', 'head') for kind in COMPARED if medians[label][kind] is None
    ]
    if missing:
        print('no measurement for: ' + ', '.join(missing))
        failed = True

    run_id = os.environ.get('GITHUB_RUN_ID')
    run_attempt = os.environ.get('GITHUB_RUN_ATTEMPT')
    base_commit = git_commit(base_root)
    head_commit = git_commit(root)
    for kind in COMPARED:
        base_med, head_med = medians['base'][kind], medians['head'][kind]
        error = None
        if base_med is None or head_med is None:
            error = 'no measurement'
        elif not args.report_only:
            try:
                check_relative(kind, head_med, base_med)
            except ValueError as relative_error:
                error = str(relative_error)
        record = {
            'schema': 'CORE_PERFORMANCE_EVIDENCE_V1',
            'workload': kind,
            'mode': 'relative',
            'commit': head_commit,
            'base_commit': base_commit,
            'run_id': run_id,
            'run_attempt': run_attempt,
            'result': 'pass' if error is None else 'fail',
            'base_median': base_med,
            'head_median': head_med,
        }
        if error is not None:
            record['error'] = error
            failed = True
        print('CORE_PERFORMANCE_EVIDENCE_V1 ' + json.dumps(record, sort_keys=True))

    if not args.report_only:
        snapshot = {
            'schema': 'CORE_PERFORMANCE_EVIDENCE_V1',
            'workload': 'snapshot',
            'mode': 'relative',
            'commit': head_commit,
            'base_commit': base_commit,
            'run_id': run_id,
            'run_attempt': run_attempt,
            'base_median': None,
            'head_median': None,
        }
        try:
            run_workload('snapshot', root)
            snapshot['result'] = 'pass'
        except ValueError as snapshot_error:
            snapshot['result'] = 'fail'
            snapshot['error'] = str(snapshot_error)
            failed = True
        print('CORE_PERFORMANCE_EVIDENCE_V1 ' + json.dumps(snapshot, sort_keys=True))

    baseline_path = args.baseline if args.baseline is not None else Path(root) / BASELINE_PATH
    try:
        baseline = load_baseline(baseline_path)
    except ValueError:
        baseline = None

    report, markdown = ab_report(
        args.base_ref, args.head_ref, base_commit, head_commit, args.repeats,
        base_samples, head_samples, errors, baseline,
        None if args.report_only else REGRESSION_THRESHOLD,
    )
    if args.report_dir is not None:
        args.report_dir.mkdir(parents=True, exist_ok=True)
        (args.report_dir / 'perf-ab.json').write_text(json.dumps(report, indent=2) + '\n')
        (args.report_dir / 'perf-ab.md').write_text(markdown)
    summary = os.environ.get('GITHUB_STEP_SUMMARY')
    if summary:
        with open(summary, 'a') as handle:
            handle.write(markdown)
    print(markdown)
    return 1 if failed else 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path('.'))
    parser.add_argument('--baseline', type=Path, default=None)
    parser.add_argument('--write-baseline', action='store_true')
    parser.add_argument('--repeats', type=int, default=REPEATS)
    parser.add_argument('--base-root', type=Path, default=None)
    parser.add_argument('--base-ref', type=str, default='')
    parser.add_argument('--head-ref', type=str, default='')
    parser.add_argument('--report-dir', type=Path, default=None)
    parser.add_argument('--report-only', action='store_true')
    args = parser.parse_args(argv)
    if args.repeats < 1:
        parser.error('--repeats must be 1 or more')
    root = str(args.root)
    if args.base_root is not None:
        return run_ab(args, root)
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
