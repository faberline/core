"""Measured values, unchanged thresholds and complete target roster controls."""
import importlib.util
from pathlib import Path
import unittest

SPEC=importlib.util.spec_from_file_location('measure',Path(__file__).parents[1]/'measure.py')
m=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(m)


def output(name):
    return f'running 1 test\ntest {name} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s\n'


class MeasurementTests(unittest.TestCase):
    h2=output('emit::py::tests::generated_python_h2c_runtime_reused_connection_perf_guard')
    peak=output('snapshot_peak_memory_grows_sublinearly')
    peak_line='CORE_SNAPSHOT_PEAK small_entries=1000 small_peak_bytes=65536 large_entries=50000 large_peak_bytes=65536 limit_bytes=262144\n'

    def test_actual_http2_measurement_and_workload_retained(self):
        result=m.measurements(self.h2,'h2c_generated_sync_reused_connection seconds=1.123456 requests=128\n','http2')
        self.assertEqual(result['measurement']['requests'],128)
        self.assertEqual(result['measurement']['seconds_exclusive_limit'],5)
        self.assertEqual(result['measurement']['seconds'],1.123456)

    def test_http2_limit_is_strict_and_cannot_change_request_count(self):
        for seconds,requests in [('5.000000',128),('9',128),('0',128),('1',127),('1',0)]:
            with self.subTest(seconds=seconds,requests=requests),self.assertRaises(ValueError):
                m.measurements(self.h2,f'h2c_generated_sync_reused_connection seconds={seconds} requests={requests}\n','http2')

    def test_missing_or_duplicate_http2_measurement_refuses(self):
        line='h2c_generated_sync_reused_connection seconds=1 requests=128\n'
        for data in ['',line+line]:
            with self.assertRaises(ValueError):m.measurements(self.h2,data,'http2')

    def test_wrong_test_cannot_supply_measurements(self):
        with self.assertRaisesRegex(ValueError,'did not execute'):
            m.measurements(output('unrelated'),'h2c_generated_sync_reused_connection seconds=1 requests=128\n','http2')

    def test_actual_snapshot_measurement_is_retained(self):
        result=m.measurements(self.peak,self.peak_line,'snapshot')
        self.assertEqual(result['measurement']['large_entries'],50000)
        self.assertEqual(result['measurement']['bytes_exclusive_limit'],262144)

    def test_memory_threshold_and_workloads_cannot_be_relaxed(self):
        for before,after in [('large_entries=50000','large_entries=1000'),('small_entries=1000','small_entries=10'),('limit_bytes=262144','limit_bytes=524288'),('large_peak_bytes=65536','large_peak_bytes=262144')]:
            with self.subTest(after=after),self.assertRaises(ValueError):
                m.measurements(self.peak,self.peak_line.replace(before,after),'snapshot')

    def test_missing_duplicate_and_unknown_measurement_refuses(self):
        for data,kind in [('', 'snapshot'),(self.peak_line*2,'snapshot'),(self.peak_line,'invented')]:
            with self.assertRaises(ValueError):m.measurements(self.peak,data,kind)

    def test_success_exit_without_full_passing_roster_cannot_qualify(self):
        for output_text in ['',self.peak.replace('0 ignored','1 ignored'),self.peak.replace('0 filtered','1 filtered')]:
            with self.assertRaises(ValueError):m.measurements(output_text,self.peak_line,'snapshot')
