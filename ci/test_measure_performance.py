import contextlib
import importlib.util
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

_SPEC = importlib.util.spec_from_file_location(
    'measure_performance', Path(__file__).with_name('measure-performance.py')
)
mod = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(mod)


def output(name):
    return (
        'running 1 test\n'
        f'test {name} ... ok\n'
        'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s\n'
    )


H2_NAME = 'emit::py::tests::h2c_connection::generated_python_h2c_runtime_reused_connection_perf_guard'
SNAP_NAME = 'snapshot_peak_memory_grows_sublinearly'
WRITE_NAME = 'write_path_latency_is_measured'
h2 = output(H2_NAME)
peak = output(SNAP_NAME)
write = output(WRITE_NAME)
good_h2_line = 'h2c_generated_sync_reused_connection seconds=1.123456 requests=128\n'
good_h2_latency_line = 'h2c_generated_sync_latency requests=128 rps=1000.000 p50_ms=1.000 p99_ms=2.000\n'
write_line = 'CORE_WRITE_PATH_LATENCY requests=500 rps=2000.000 p50_ms=0.400 p99_ms=0.900\n'
peak_line = (
    'CORE_SNAPSHOT_PEAK small_entries=1000 small_peak_bytes=65536 '
    'large_entries=50000 large_peak_bytes=65536 limit_bytes=262144\n'
)
BASELINE = {'requests_per_second': 1000.0, 'p50_ms': 1.0, 'p99_ms': 2.0}
WRITE_BASELINE = {'requests_per_second': 2000.0, 'p50_ms': 0.4, 'p99_ms': 0.9}
BASELINE_FILE = {'http2': BASELINE, 'raft_write': WRITE_BASELINE}


class Http2Tests(unittest.TestCase):
    def test_http2_pass(self):
        result = mod.check_http2(h2, good_h2_line)
        self.assertEqual(result['workload'], 'http2')
        self.assertEqual(result['measurement']['seconds'], 1.123456)
        self.assertEqual(result['measurement']['requests'], 128)
        self.assertEqual(result['measurement']['seconds_exclusive_limit'], 5.0)

    def test_http2_limit_is_strict(self):
        for seconds, requests in (('5.000000', 128), ('9', 128), ('0', 128), ('1', 127), ('1', 0)):
            line = f'h2c_generated_sync_reused_connection seconds={seconds} requests={requests}\n'
            with self.subTest(seconds=seconds, requests=requests):
                with self.assertRaises(ValueError):
                    mod.check_http2(h2, line)

    def test_http2_missing_or_duplicate_line(self):
        with self.assertRaises(ValueError):
            mod.check_http2(h2, '')
        with self.assertRaises(ValueError):
            mod.check_http2(h2, good_h2_line * 2)

    def test_http2_wrong_test_name(self):
        with self.assertRaisesRegex(ValueError, 'did not execute'):
            mod.check_http2(output('unrelated'), good_h2_line)

    def test_http2_ignored_test_refused(self):
        stdout = (
            'running 1 test\n'
            f'test {H2_NAME} ... ignored\n'
            'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n'
        )
        with self.assertRaises(ValueError):
            mod.check_http2(stdout, good_h2_line)

    def test_original_thresholds_unchanged(self):
        with self.assertRaises(ValueError):
            mod.check_http2(h2, 'h2c_generated_sync_reused_connection seconds=5.000000 requests=128\n')


class SnapshotTests(unittest.TestCase):
    def test_snapshot_pass(self):
        result = mod.check_snapshot(peak, peak_line)
        self.assertEqual(result['workload'], 'snapshot')
        self.assertEqual(result['measurement']['large_entries'], 50000)
        self.assertEqual(result['measurement']['bytes_exclusive_limit'], 262144)

    def test_snapshot_zero_peaks_are_valid(self):
        line = peak_line.replace('peak_bytes=65536', 'peak_bytes=0')
        result = mod.check_snapshot(peak, line)
        self.assertEqual(result['measurement']['bytes_exclusive_limit'], 262144)

    def test_snapshot_cannot_be_relaxed(self):
        replacements = (
            ('large_entries=50000', 'large_entries=1000'),
            ('small_entries=1000', 'small_entries=10'),
            ('limit_bytes=262144', 'limit_bytes=524288'),
            ('large_peak_bytes=65536', 'large_peak_bytes=262144'),
        )
        for old, new in replacements:
            with self.subTest(old=old):
                with self.assertRaises(ValueError):
                    mod.check_snapshot(peak, peak_line.replace(old, new))

    def test_snapshot_missing_or_duplicate_line(self):
        with self.assertRaises(ValueError):
            mod.check_snapshot(peak, '')
        with self.assertRaises(ValueError):
            mod.check_snapshot(peak, peak_line * 2)


class RegressionTests(unittest.TestCase):
    def test_equal_to_baseline_passes(self):
        self.assertIsNone(mod.check_regression('http2', dict(BASELINE, requests=128), BASELINE))

    def test_seeded_regression_fails(self):
        # 50 percent worse than the baseline: half the requests per second, 1.5 times the latency.
        worse = {'requests_per_second': 500.0, 'p50_ms': 1.5, 'p99_ms': 3.0}
        with self.assertRaisesRegex(ValueError, 'regression'):
            mod.check_regression('http2', worse, BASELINE)

    def test_seeded_regression_fails_per_field(self):
        cases = (
            {'requests_per_second': 500.0, 'p50_ms': 1.0, 'p99_ms': 2.0},
            {'requests_per_second': 1000.0, 'p50_ms': 1.5, 'p99_ms': 2.0},
            {'requests_per_second': 1000.0, 'p50_ms': 1.0, 'p99_ms': 3.0},
        )
        for measured in cases:
            with self.subTest(measured=measured):
                with self.assertRaisesRegex(ValueError, 'regression'):
                    mod.check_regression('http2', measured, BASELINE)

    def test_exactly_30_percent_passes_and_31_fails(self):
        self.assertIsNone(mod.check_regression(
            'http2', {'requests_per_second': 700.0, 'p50_ms': 1.3, 'p99_ms': 2.6}, BASELINE))
        for measured in (
            {'requests_per_second': 690.0, 'p50_ms': 1.0, 'p99_ms': 2.0},
            {'requests_per_second': 1000.0, 'p50_ms': 1.31, 'p99_ms': 2.0},
            {'requests_per_second': 1000.0, 'p50_ms': 1.0, 'p99_ms': 2.62},
        ):
            with self.subTest(measured=measured):
                with self.assertRaisesRegex(ValueError, 'regression'):
                    mod.check_regression('http2', measured, BASELINE)

    def test_better_than_baseline_passes(self):
        self.assertIsNone(mod.check_regression(
            'http2', {'requests_per_second': 2000.0, 'p50_ms': 0.5, 'p99_ms': 1.0}, BASELINE))

    def test_threshold_is_one_named_constant(self):
        self.assertEqual(mod.REGRESSION_THRESHOLD, 0.30)

    def test_parse_latency_good_and_bad(self):
        parsed = mod.parse_latency(mod.H2_LATENCY_RE, good_h2_latency_line, 'http2')
        self.assertEqual(parsed['p99_ms'], 2.0)
        self.assertEqual(parsed['requests'], 128)
        with self.assertRaises(ValueError):
            mod.parse_latency(mod.H2_LATENCY_RE, '', 'http2')
        with self.assertRaises(ValueError):
            mod.parse_latency(mod.H2_LATENCY_RE, good_h2_latency_line * 2, 'http2')
        with self.assertRaises(ValueError):
            mod.parse_latency(mod.H2_LATENCY_RE, good_h2_latency_line.replace('p50_ms=1.000', 'p50_ms=0'), 'http2')

    def test_load_baseline(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'baseline.json'
            with self.assertRaisesRegex(ValueError, 'missing performance baseline'):
                mod.load_baseline(path)
            path.write_text(json.dumps(BASELINE_FILE))
            self.assertEqual(mod.load_baseline(path), BASELINE_FILE)
            path.write_text(json.dumps({'http2': BASELINE}))
            with self.assertRaisesRegex(ValueError, 'raft_write'):
                mod.load_baseline(path)


class WritePathTests(unittest.TestCase):
    def test_write_path_pass(self):
        result = mod.check_write_path(write, write_line)
        self.assertEqual(result['workload'], 'raft_write')
        self.assertEqual(result['measurement']['requests_per_second'], 2000.0)

    def test_write_path_missing_line(self):
        with self.assertRaises(ValueError):
            mod.check_write_path(write, '')

    def test_write_path_wrong_test_name(self):
        with self.assertRaisesRegex(ValueError, 'did not execute'):
            mod.check_write_path(output('unrelated'), write_line)


class MainTests(unittest.TestCase):
    def test_main_reports_failure_exit_code(self):
        def fake_run(command, **kwargs):
            if command[0] == 'git':
                return mock.Mock(returncode=0, stdout='abc123\n', stderr='')
            if '--test' in command and 'snapshot_peak_memory' in command:
                return mock.Mock(returncode=101, stdout='', stderr='')
            if '--test' in command and 'write_path_latency' in command:
                return mock.Mock(returncode=0, stdout=write, stderr=write_line)
            return mock.Mock(returncode=0, stdout=h2, stderr=good_h2_line + good_h2_latency_line)

        with tempfile.TemporaryDirectory() as tmp:
            baseline = Path(tmp) / 'baseline.json'
            baseline.write_text(json.dumps(BASELINE_FILE))
            buf = io.StringIO()
            with mock.patch.object(mod.subprocess, 'run', side_effect=fake_run):
                with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(io.StringIO()):
                    code = mod.main(['--root', '.', '--baseline', str(baseline)])
        self.assertEqual(code, 1)
        lines = [l for l in buf.getvalue().splitlines() if l.startswith('CORE_PERFORMANCE_EVIDENCE_V1 ')]
        self.assertEqual(len(lines), 3)
        self.assertEqual(sum('"result": "pass"' in l for l in lines), 2)
        self.assertEqual(sum('"result": "fail"' in l for l in lines), 1)

    def test_main_write_baseline(self):
        def fake_run(command, **kwargs):
            if command[0] == 'git':
                return mock.Mock(returncode=0, stdout='abc123\n', stderr='')
            if '--test' in command and 'snapshot_peak_memory' in command:
                return mock.Mock(returncode=0, stdout=peak, stderr=peak_line)
            if '--test' in command and 'write_path_latency' in command:
                return mock.Mock(returncode=0, stdout=write, stderr=write_line)
            return mock.Mock(returncode=0, stdout=h2, stderr=good_h2_line + good_h2_latency_line)

        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'b.json'
            with mock.patch.object(mod.subprocess, 'run', side_effect=fake_run):
                with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    code = mod.main(['--root', '.', '--write-baseline', '--baseline', str(path)])
            self.assertEqual(code, 0)
            data = json.loads(path.read_text())
        self.assertEqual(sorted(data), ['http2', 'raft_write'])
        for kind in ('http2', 'raft_write'):
            self.assertEqual(sorted(data[kind]), ['p50_ms', 'p99_ms', 'requests_per_second'])
        self.assertEqual(data['http2'], BASELINE)
        self.assertEqual(data['raft_write'], WRITE_BASELINE)


def latency_line(rps, p50, p99):
    return f'CORE_WRITE_PATH_LATENCY requests=500 rps={rps} p50_ms={p50} p99_ms={p99}\n'


def h2_latency(rps, p50, p99):
    return f'h2c_generated_sync_latency requests=128 rps={rps} p50_ms={p50} p99_ms={p99}\n'


def write_runs(samples):
    """A fake subprocess.run for the raft_write command: one (rps, p50, p99) per call."""
    outputs = iter(samples)

    def fake_run(command, **kwargs):
        rps, p50, p99 = next(outputs)
        return mock.Mock(returncode=0, stdout=write, stderr=latency_line(rps, p50, p99))

    return fake_run


def all_ok(counts):
    """A fake subprocess.run for every workload; counts the raft_write runs."""

    def fake_run(command, **kwargs):
        if command[0] == 'git':
            return mock.Mock(returncode=0, stdout='abc123\n', stderr='')
        if 'write_path_latency' in command:
            counts.append('write')
            return mock.Mock(returncode=0, stdout=write, stderr=write_line)
        if 'snapshot_peak_memory' in command:
            return mock.Mock(returncode=0, stdout=peak, stderr=peak_line)
        return mock.Mock(returncode=0, stdout=h2, stderr=good_h2_line + good_h2_latency_line)

    return fake_run


class MedianTests(unittest.TestCase):
    def test_median_each_field_alone(self):
        samples = [
            dict(requests=500, requests_per_second=r, p50_ms=p, p99_ms=q)
            for r, p, q in ((1000.0, 0.5, 9.0), (1400.0, 0.7, 1.0), (1200.0, 0.6, 1.2))
        ]
        self.assertEqual(
            mod.median_latency(samples),
            {'requests': 500, 'requests_per_second': 1200.0, 'p50_ms': 0.6, 'p99_ms': 1.2},
        )

    def test_median_empty_fails(self):
        with self.assertRaisesRegex(ValueError, 'no latency samples'):
            mod.median_latency([])

    def test_repeats_default_is_five_and_odd(self):
        self.assertEqual(mod.REPEATS, 5)
        self.assertEqual(mod.REPEATS % 2, 1)


class RepeatTests(unittest.TestCase):
    def test_one_outlier_passes(self):
        fake = write_runs([
            (1400.0, 0.67, 1.2), (878.0, 0.79, 7.5), (1410.0, 0.68, 1.3),
            (1390.0, 0.66, 1.2), (1405.0, 0.69, 1.3),
        ])
        with mock.patch.object(mod.subprocess, 'run', side_effect=fake):
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                measurement = mod.run_repeated('raft_write', '.', 5)['measurement']
        self.assertEqual(measurement['requests_per_second'], 1400.0)
        self.assertEqual(measurement['p50_ms'], 0.68)
        self.assertEqual(measurement['p99_ms'], 1.3)
        self.assertEqual(measurement['repeats'], 5)
        self.assertEqual(len(measurement['samples']), 5)
        stored = {'requests_per_second': 1420.024, 'p50_ms': 0.671, 'p99_ms': 1.277}
        self.assertIsNone(mod.check_regression('raft_write', measurement, stored))

    def test_real_regression_still_fails(self):
        fake = write_runs([(700.0, 0.9, 2.0)] * 5)
        with mock.patch.object(mod.subprocess, 'run', side_effect=fake):
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                measurement = mod.run_repeated('raft_write', '.', 5)['measurement']
        with self.assertRaisesRegex(ValueError, 'regression'):
            mod.check_regression('raft_write', measurement, WRITE_BASELINE)

    def test_http2_keeps_worst_seconds_and_hard_limit(self):
        seconds = iter(['0.03', '0.04', '0.05', '0.03', '0.03'])

        def fake_run(command, **kwargs):
            s = next(seconds)
            return mock.Mock(
                returncode=0,
                stdout=h2,
                stderr=(
                    f'h2c_generated_sync_reused_connection seconds={s} requests=128\n'
                    + h2_latency(1000.0, 1.0, 2.0)
                ),
            )

        with mock.patch.object(mod.subprocess, 'run', side_effect=fake_run):
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                measurement = mod.run_repeated('http2', '.', 5)['measurement']
        self.assertEqual(measurement['seconds'], 0.05)
        self.assertEqual(measurement['seconds_exclusive_limit'], 5.0)
        self.assertEqual(measurement['latency']['requests_per_second'], 1000.0)
        self.assertEqual(measurement['latency']['p50_ms'], 1.0)
        self.assertEqual(measurement['latency']['p99_ms'], 2.0)
        self.assertEqual(measurement['repeats'], 5)

    def test_failed_repeat_fails_at_once(self):
        calls = []

        def fake_run(command, **kwargs):
            calls.append(command)
            code = 101 if len(calls) == 3 else 0
            return mock.Mock(returncode=code, stdout=write, stderr=write_line)

        with mock.patch.object(mod.subprocess, 'run', side_effect=fake_run):
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(ValueError, 'measured target failed'):
                    mod.run_repeated('raft_write', '.', 5)
        self.assertEqual(len(calls), 3)


class MainRepeatTests(unittest.TestCase):
    def run_main(self, argv):
        counts = []
        out = io.StringIO()
        with tempfile.TemporaryDirectory() as tmp:
            baseline = Path(tmp) / 'baseline.json'
            baseline.write_text(json.dumps(BASELINE_FILE))
            with mock.patch.object(mod.subprocess, 'run', side_effect=all_ok(counts)):
                with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
                    code = mod.main(['--root', '.', '--baseline', str(baseline)] + argv)
        return code, counts, out.getvalue()

    def test_main_repeats_option(self):
        code, counts, _ = self.run_main(['--repeats', '3'])
        self.assertEqual(code, 0)
        self.assertEqual(len(counts), 3)

    def test_main_refuses_zero_repeats(self):
        with contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                mod.main(['--repeats', '0'])

    def test_evidence_lists_samples(self):
        _, _, out = self.run_main(['--repeats', '3'])
        lines = [
            l for l in out.splitlines()
            if l.startswith('CORE_PERFORMANCE_EVIDENCE_V1 ') and '"workload": "raft_write"' in l
        ]
        self.assertEqual(len(lines), 1)
        record = json.loads(lines[0].split(' ', 1)[1])
        self.assertEqual(len(record['measurement']['samples']), 3)
        self.assertEqual(record['result'], 'pass')


if __name__ == '__main__':
    unittest.main()
