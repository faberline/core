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
good_h2_line = 'h2c_generated_sync_reused_connection seconds=1.123456 requests=128\n'
good_h2_latency_line = 'h2c_generated_sync_latency requests=128 rps=1000.000 p50_ms=1.000 p99_ms=2.000\n'
write_line = 'CORE_WRITE_PATH_LATENCY requests=500 rps=2000.000 p50_ms=0.400 p99_ms=0.900\n'
peak_line = (
    'CORE_SNAPSHOT_PEAK small_entries=1000 small_peak_bytes=65536 '
    'large_entries=50000 large_peak_bytes=65536 limit_bytes=262144\n'
)
BASELINE = {'requests_per_second': 1000.0, 'p50_ms': 1.0, 'p99_ms': 2.0}
BASELINE_FILE = {'http2': BASELINE, 'raft_write': BASELINE}


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
        # 50 percent worse than the baseline in every field.
        measured = {'requests_per_second': 500.0, 'p50_ms': 1.5, 'p99_ms': 3.0}
        with self.assertRaisesRegex(ValueError, 'regression'):
            mod.check_regression('http2', measured, BASELINE)

    def test_seeded_regression_fails_per_field(self):
        cases = (
            ({'requests_per_second': 500.0, 'p50_ms': 1.0, 'p99_ms': 2.0}, 'requests_per_second'),
            ({'requests_per_second': 1000.0, 'p50_ms': 1.5, 'p99_ms': 2.0}, 'p50_ms'),
            ({'requests_per_second': 1000.0, 'p50_ms': 1.0, 'p99_ms': 3.0}, 'p99_ms'),
        )
        for measured, field in cases:
            with self.subTest(field=field):
                with self.assertRaisesRegex(ValueError, f'{field}'):
                    mod.check_regression('http2', measured, BASELINE)

    def test_exactly_30_percent_passes_and_31_fails(self):
        mod.check_regression('http2', {'requests_per_second': 700.0, 'p50_ms': 1.3, 'p99_ms': 2.6}, BASELINE)
        for measured in (
            {'requests_per_second': 690.0, 'p50_ms': 1.0, 'p99_ms': 2.0},
            {'requests_per_second': 1000.0, 'p50_ms': 1.31, 'p99_ms': 2.0},
            {'requests_per_second': 1000.0, 'p50_ms': 1.0, 'p99_ms': 2.62},
        ):
            with self.subTest(measured=measured):
                with self.assertRaises(ValueError):
                    mod.check_regression('http2', measured, BASELINE)

    def test_better_than_baseline_passes(self):
        mod.check_regression('http2', {'requests_per_second': 2000.0, 'p50_ms': 0.5, 'p99_ms': 1.0}, BASELINE)

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
            mod.parse_latency(
                mod.H2_LATENCY_RE,
                good_h2_latency_line.replace('p50_ms=1.000', 'p50_ms=0'),
                'http2',
            )

    def test_original_thresholds_unchanged(self):
        with self.assertRaises(ValueError):
            mod.check_http2(h2, 'h2c_generated_sync_reused_connection seconds=5.000000 requests=128\n')

    def test_load_baseline(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'baseline.json'
            with self.assertRaisesRegex(ValueError, 'missing performance baseline'):
                mod.load_baseline(path)
            path.write_text(json.dumps(BASELINE_FILE))
            self.assertEqual(mod.load_baseline(path), BASELINE_FILE)
            path.write_text(json.dumps({'http2': BASELINE}))
            with self.assertRaises(ValueError):
                mod.load_baseline(path)


def fake_run_for(write_stdout=None):
    """A subprocess.run stand-in: every workload passes unless overridden."""

    def fake_run(command, **kwargs):
        if command[0] == 'git':
            return mock.Mock(returncode=0, stdout='abc123\n', stderr='')
        if 'write_path_latency' in command:
            return mock.Mock(returncode=0, stdout=output(WRITE_NAME), stderr=write_line)
        if '--test' in command and 'snapshot_peak_memory' in command:
            return mock.Mock(returncode=0, stdout=peak, stderr=peak_line)
        return mock.Mock(returncode=0, stdout=h2, stderr=good_h2_line + good_h2_latency_line)

    return fake_run


class MainTests(unittest.TestCase):
    def test_main_reports_failure_exit_code(self):
        def fake_run(command, **kwargs):
            if command[0] == 'git':
                return mock.Mock(returncode=0, stdout='abc123\n', stderr='')
            if '--test' in command and 'snapshot_peak_memory' in command:
                return mock.Mock(returncode=101, stdout='', stderr='')
            if 'write_path_latency' in command:
                return mock.Mock(returncode=0, stdout=output(WRITE_NAME), stderr=write_line)
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
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'b.json'
            buf = io.StringIO()
            with mock.patch.object(mod.subprocess, 'run', side_effect=fake_run_for()):
                with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(io.StringIO()):
                    code = mod.main(['--root', '.', '--write-baseline', '--baseline', str(path)])
            self.assertEqual(code, 0)
            written = json.loads(path.read_text())
        self.assertEqual(set(written), {'http2', 'raft_write'})
        for kind in ('http2', 'raft_write'):
            self.assertEqual(
                set(written[kind]), {'requests_per_second', 'p50_ms', 'p99_ms'}
            )
        self.assertEqual(written['http2'], BASELINE)
        self.assertEqual(written['raft_write'], {'requests_per_second': 2000.0, 'p50_ms': 0.4, 'p99_ms': 0.9})

    def test_main_missing_baseline_fails_compared_workloads(self):
        with tempfile.TemporaryDirectory() as tmp:
            buf = io.StringIO()
            with mock.patch.object(mod.subprocess, 'run', side_effect=fake_run_for()):
                with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(io.StringIO()):
                    code = mod.main(['--root', '.', '--baseline', str(Path(tmp) / 'absent.json')])
        self.assertEqual(code, 1)
        lines = [l for l in buf.getvalue().splitlines() if l.startswith('CORE_PERFORMANCE_EVIDENCE_V1 ')]
        self.assertEqual(len(lines), 3)
        self.assertEqual(sum('missing performance baseline' in l for l in lines), 2)


if __name__ == '__main__':
    unittest.main()
