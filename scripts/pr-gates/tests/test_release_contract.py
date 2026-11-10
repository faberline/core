"""Read-only release contract checks; never tag, edit or call GitHub."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('release_contract', Path(__file__).parents[1] / 'release_contract.py')
rc = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(rc)

REPO_ROOT = Path(__file__).parents[3]
GOAL = 'Shared correctness, truthful observability and bounded service foundations'
GOAL_SHA = 'ee15452ef357fd7f11f19ea5e9b7f1b3091c276210428aa27ab04f3470e14df0'
CARGO = '[workspace.package]\nversion = "0.5.0"\n'


def contract():
    return {
        'version': 1,
        'repo': 'faberline/core',
        'ready_rule': 'all-milestone-issues-closed',
        'full_test_gate': 'qualification',
        'milestones': {
            '8': {
                'title': 'core@0.5.0',
                'version': '0.5.0',
                'tag': 'v0.5.0',
                'goal': GOAL,
                'goal_sha256': GOAL_SHA,
                'issues': [2, 7, 8],
                'landed_prs': {},
            }
        },
    }


def gates():
    return {'qualification': {'rules': [
        {'paths': ['**'], 'require': ['unit-0', 'functional-0', 'measured-performance']}
    ]}}


def runs():
    return [{'name': n, 'conclusion': 'success', 'tests': 5, 'skipped': 0}
            for n in ('unit-0', 'functional-0', 'measured-performance')]


class ContractShapeTests(unittest.TestCase):
    def test_valid_contract_loads(self):
        self.assertIsNone(rc.validate_contract(contract()))

    def test_real_file_is_valid(self):
        loaded = rc.load_contract(REPO_ROOT / '.github' / 'aw-release.json')
        self.assertEqual(loaded['milestones']['8']['version'], '0.5.0')

    def test_goal_identity_mismatch_refuses(self):
        c = contract()
        c['milestones']['8']['goal'] = 'Other goal'
        with self.assertRaisesRegex(ValueError, 'goal identity'):
            rc.validate_contract(c)

    def test_tag_must_match_version(self):
        c = contract()
        c['milestones']['8']['tag'] = 'v0.5.1'
        with self.assertRaisesRegex(ValueError, 'milestone 8 tag'):
            rc.validate_contract(c)

    def test_empty_milestones_refuses(self):
        c = contract()
        c['milestones'] = {}
        with self.assertRaisesRegex(ValueError, 'empty milestones'):
            rc.validate_contract(c)

    def test_unknown_landed_issue_refuses(self):
        c = contract()
        c['milestones']['8']['landed_prs'] = {'999': [1]}
        with self.assertRaisesRegex(ValueError, 'landed_prs'):
            rc.validate_contract(c)


class VersionTests(unittest.TestCase):
    def test_matching_version_has_no_hold(self):
        self.assertEqual(rc.version_holds(contract()['milestones']['8'], CARGO), [])

    def test_goal_version_mismatch_is_a_hold(self):
        cargo = '[workspace.package]\nversion = "0.4.14"\n'
        self.assertEqual(rc.version_holds(contract()['milestones']['8'], cargo),
                         ['version-mismatch: contract 0.5.0, Cargo.toml 0.4.14'])


class EvidenceTests(unittest.TestCase):
    def test_complete_evidence_has_no_hold(self):
        self.assertEqual(len(runs()), 3)
        self.assertEqual(rc.evidence_holds(rc.required_checks(gates()), runs()), [])

    def test_missing_check_is_a_hold(self):
        partial = [r for r in runs() if r['name'] != 'functional-0']
        self.assertEqual(rc.evidence_holds(rc.required_checks(gates()), partial),
                         ['missing-check: functional-0'])

    def test_partial_evidence_is_a_hold(self):
        bad = runs()
        bad[0]['conclusion'] = 'failure'
        bad[1]['tests'] = 0
        bad[2]['skipped'] = 1
        self.assertEqual(rc.evidence_holds(rc.required_checks(gates()), bad), [
            'failed-check: unit-0', 'zero-tests: functional-0', 'skipped-tests: measured-performance'])

    def test_required_checks_without_rule_refuses(self):
        with self.assertRaisesRegex(ValueError, 'no full-test rule'):
            rc.required_checks({'qualification': {'rules': []}})


class PromotionTests(unittest.TestCase):
    def test_same_commit_promotes(self):
        rc_tag = {'tag': 'v0.5.0-rc.1', 'sha': 'a' * 40}
        formal = {'tag': 'v0.5.0', 'sha': 'a' * 40}
        self.assertEqual(rc.promotion_holds(rc_tag, formal), [])

    def test_rc_to_formal_other_commit_is_a_hold(self):
        holds = rc.promotion_holds({'tag': 'v0.5.0-rc.1', 'sha': 'a' * 40},
                                   {'tag': 'v0.5.0', 'sha': 'b' * 40})
        self.assertEqual(len(holds), 1)
        self.assertTrue(holds[0].startswith('promotion-sha-mismatch'))


class StatusTests(unittest.TestCase):
    def test_open_issues_block_release(self):
        result = rc.release_status(contract(), '8', 3, [2, 7], CARGO, gates(), runs())
        self.assertTrue(result['holds'][0].startswith('open-milestone-issues: 3'))
        self.assertFalse(result['ready'])

    def test_all_closed_with_full_evidence_is_ready(self):
        c = contract()
        c['milestones']['8']['issues'] = [2, 7]
        c['milestones']['8']['landed_prs'] = {'2': [10], '7': [11]}
        result = rc.release_status(c, '8', 0, [2, 7], CARGO, gates(), runs())
        self.assertTrue(result['ready'])
        self.assertEqual(result['holds'], [])

    def test_missing_landed_pr_is_a_hold(self):
        result = rc.release_status(contract(), '8', 0, [2], CARGO, gates(), runs())
        self.assertIn('missing-landed-pr: #2', result['holds'])

    def test_unknown_milestone_refuses(self):
        with self.assertRaisesRegex(ValueError, 'unknown milestone'):
            rc.release_status(contract(), '99', 0, [], CARGO, gates(), runs())


class PromotionFormatTests(unittest.TestCase):
    def test_landing_holds_lists_each_closed_issue_once(self):
        self.assertEqual(rc.landing_holds(contract()['milestones']['8'], [2, 7]),
                         ['missing-landed-pr: #2', 'missing-landed-pr: #7'])


class CliTests(unittest.TestCase):
    def _root(self, tmp, milestone_contract=None):
        base = Path(tmp)
        (base / '.github').mkdir()
        (base / '.github' / 'aw-release.json').write_text(json.dumps(milestone_contract or contract()) + '\n')
        (base / 'Cargo.toml').write_text(CARGO)
        (base / '.github' / 'issue-ci-required-gates.json').write_text(json.dumps(gates()) + '\n')
        return base

    def test_check_exits_1_while_holds_remain(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self._root(tmp)
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                code = rc.main(['check', '--root', str(root), '--milestone', '8',
                                '--open-issues', '1', '--closed', '2'])
            self.assertEqual(code, 1)
            self.assertFalse(json.loads(out.getvalue())['ready'])

    def test_check_unknown_milestone_exits_2(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self._root(tmp)
            with contextlib.redirect_stderr(io.StringIO()):
                code = rc.main(['check', '--root', str(root), '--milestone', '99',
                                '--open-issues', '0', '--closed', ''])
            self.assertEqual(code, 2)


if __name__ == '__main__':
    unittest.main()
