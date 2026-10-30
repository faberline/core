"""Verify the real workflow's stage behavior and dependency ordering."""
import os
from pathlib import Path
import re
import subprocess
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = (ROOT / '.github/workflows/ci.yml').read_text()


def job(name):
    match = re.search(r'^  ' + re.escape(name) + r':\n(.*?)(?=^  [a-z][a-z0-9-]*:\n|\Z)',
                      WORKFLOW, re.M | re.S)
    if match is None:
        raise AssertionError('missing job: ' + name)
    return match.group(1)


def dependencies(name):
    match = re.search(r'^    needs: (.+)$', job(name), re.M)
    if match is None:
        return set()
    return set(re.findall(r'[a-z][a-z0-9-]*', match.group(1)))


class CISequenceTests(unittest.TestCase):
    def test_unit_then_integration_then_performance(self):
        self.assertEqual(dependencies('unit-stage'), {'unit-plan', 'workspace-format', 'workspace-unit-static',
                                                       'source-issue-47-unit'})
        self.assertIn('unit-stage', dependencies('crate-gates'))
        self.assertIn('unit-stage', dependencies('source-issue-47-integration'))
        self.assertTrue({'unit-stage', 'unit-plan'} <= dependencies('workspace-contracts'))
        self.assertTrue({'unit-stage', 'crate-gates', 'source-issue-47-integration', 'workspace-contracts'}
                        <= dependencies('measured-performance'))
        self.assertIn('gate: [proposal, clippy]', job('source-issue-47-unit'))
        self.assertIn('cargo clippy --locked -p raft-runtime --all-targets -- -D warnings',
                      job('source-issue-47-unit'))
        self.assertIn('cargo test --locked -p raft-core -p raft-runtime --no-fail-fast',
                      job('source-issue-47-integration'))

    def test_pr_requires_every_unit_job_to_succeed(self):
        body = job('unit-stage')
        match = re.search(r'^        run: \|\n((?:^          .*\n)+)', body, re.M)
        self.assertIsNotNone(match)
        script = textwrap.dedent(match.group(1))
        for event, inventory, formatting, units, issue_unit, expected in [
            ('pull_request', 'success', 'success', 'success', 'success', 0),
            ('pull_request', 'failure', 'success', 'success', 'success', 1),
            ('pull_request', 'success', 'skipped', 'success', 'success', 1),
            ('pull_request', 'success', 'success', 'cancelled', 'success', 1),
            ('pull_request', 'success', 'success', 'success', 'failure', 1),
            ('push', 'skipped', 'skipped', 'skipped', 'skipped', 0),
            ('workflow_dispatch', 'skipped', 'skipped', 'skipped', 'skipped', 0),
        ]:
            with self.subTest(event=event, inventory=inventory, formatting=formatting, units=units):
                env = dict(os.environ, EVENT_NAME=event, INVENTORY=inventory,
                           FORMATTING=formatting, UNIT_STATIC=units, ISSUE_47_UNIT=issue_unit)
                result = subprocess.run(['bash', '-c', script], env=env, capture_output=True)
                self.assertEqual(result.returncode, expected)


if __name__ == '__main__':
    unittest.main()
