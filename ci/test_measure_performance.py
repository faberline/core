import contextlib
import importlib.util
import io
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
h2 = output(H2_NAME)
peak = output(SNAP_NAME)
good_h2_line = 'h2c_generated_sync_reused_connection seconds=1.123456 requests=128\n'
peak_line = (
    'CORE_SNAPSHOT_PEAK small_entries=1000 small_peak_bytes=65536 '
    'large_entries=50000 large_peak_bytes=65536 limit_bytes=262144\n'
)


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


class MainTests(unittest.TestCase):
    def test_main_reports_failure_exit_code(self):
        def fake_run(command, **kwargs):
            if command[0] == 'git':
                return mock.Mock(returncode=0, stdout='abc123\n', stderr='')
            if '--test' in command and 'snapshot_peak_memory' in command:
                return mock.Mock(returncode=101, stdout='', stderr='')
            return mock.Mock(returncode=0, stdout=h2, stderr=good_h2_line)

        buf = io.StringIO()
        with mock.patch.object(mod.subprocess, 'run', side_effect=fake_run):
            with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(io.StringIO()):
                code = mod.main(['--root', '.'])
        self.assertEqual(code, 1)
        lines = [l for l in buf.getvalue().splitlines() if l.startswith('CORE_PERFORMANCE_EVIDENCE_V1 ')]
        self.assertEqual(len(lines), 2)
        self.assertTrue(any('"result": "pass"' in l for l in lines))
        self.assertTrue(any('"result": "fail"' in l for l in lines))


if __name__ == '__main__':
    unittest.main()
