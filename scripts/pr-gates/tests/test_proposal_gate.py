"""The explicit filtered source gate must prove its complete selected roster."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('proposal_gate', Path(__file__).parents[1] / 'proposal_gate.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class ProposalGateTests(unittest.TestCase):
    def test_filtered_command_runs_every_listed_proposal_test(self):
        expected = gate.selected_roster('proposal::one: test\nproposal::two: test\n\n2 tests, 0 benchmarks\n')
        output = ('running 2 tests\ntest proposal::one ... ok\ntest proposal::two ... ok\n'
                  'test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 1.0s\n')
        self.assertEqual(gate.verify_execution(output, expected)['roster'], ['proposal::one', 'proposal::two'])

    def test_empty_duplicate_nonpassing_and_partial_evidence_refuses(self):
        for listing in ('', 'proposal::one: test\nproposal::one: test\n', 'unrelated::one: test\n'):
            with self.subTest(listing=listing), self.assertRaises(ValueError):
                gate.selected_roster(listing)
        expected = {'proposal::one', 'proposal::two'}
        bad = [
            'running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out;\n',
            'running 2 tests\ntest proposal::one ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 40 filtered out;\n',
            'running 2 tests\ntest proposal::one ... ok\ntest proposal::two ... ignored\ntest result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 40 filtered out;\n',
            'running 2 tests\ntest proposal::one ... ok\ntest proposal::two ... FAILED\ntest result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 40 filtered out;\n',
            'running 2 tests\ntest proposal::one ... ok\ntest proposal::one ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 40 filtered out;\n',
        ]
        for output in bad:
            with self.subTest(output=output), self.assertRaises(ValueError):
                gate.verify_execution(output, expected)
