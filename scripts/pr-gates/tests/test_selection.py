import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[3]
def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module
selection=load('core_shared_selection',ROOT/'scripts/pr-gates/selection.py')
shared=load('core_generated_test_plan',ROOT/'scripts/issue-ci/test_plan.py')
CONFIG=json.loads((ROOT/'.github/issue-ci-required-gates.json').read_text())
HEAD,BASE='a'*40,'b'*40


def issue_and_policy():
    policy=copy.deepcopy(CONFIG)
    issue=dict(number=84,state='open',title='Synthetic contract',body='Preserve all tests',labels=[],
               url='https://api.github.com/repos/faberline/core/issues/84',repository_url='https://api.github.com/repos/faberline/core')
    for field in ('title','body'):
        policy['qualification']['contracts']['84'][field+'_sha256']=hashlib.sha256(issue[field].encode()).hexdigest()
    pull=dict(number=85,state='open',draft=False,body='Closes #84',changed_files=1,head=dict(sha=HEAD,repo={'full_name':'faberline/core'}),
              base=dict(sha=BASE,repo={'full_name':'faberline/core'}))
    return policy,issue,pull


class SelectionTests(unittest.TestCase):
    def test_dev_and_release_retain_all_current_required_case_ids(self):
        policy,issue,pull=issue_and_policy()
        diff=selection.build(shared,policy,'diff','faberline/core',HEAD,BASE,pull,[dict(filename='crates/raft-core/src/lib.rs',status='modified')],issue)
        full=selection.build(shared,policy,'all','faberline/core',HEAD,BASE)
        self.assertEqual(diff['requested_mode'],'diff');self.assertEqual(full['requested_mode'],'all')
        self.assertEqual(diff['test_plan']['selected'],full['test_plan']['selected'])
        self.assertEqual(len(full['test_plan']['selected']),145)

    def test_protected_proposal_runs_full_tests_but_grants_no_qualification(self):
        policy,issue,pull=issue_and_policy()
        result=selection.build(shared,policy,'diff','faberline/core',HEAD,BASE,pull,[dict(filename='.github/workflows/development-ci.yml',status='modified')],issue)
        self.assertTrue(result['pending_protected_review'])
        self.assertEqual(result['test_plan']['mode'],'all');self.assertEqual(result['qualification'],'not_evaluated')
        self.assertEqual(len(result['test_plan']['selected']),145)

    def test_unknown_issue_requirement_is_not_erased_by_full_fallback(self):
        policy,issue,pull=issue_and_policy();policy['qualification']['contracts']['84']['unresolved']=['Need a real product assertion']
        with self.assertRaisesRegex(ValueError,'unresolved'):
            selection.build(shared,policy,'diff','faberline/core',HEAD,BASE,pull,[dict(filename='scripts/pr-gates/plan.py',status='modified')],issue)

    def test_full_execution_fallback_does_not_hide_a_moved_pr(self):
        policy,issue,pull=issue_and_policy();pull['head']['sha']='c'*40
        with self.assertRaisesRegex(ValueError,'PR moved'):
            selection.build(shared,policy,'diff','faberline/core',HEAD,BASE,pull,[dict(filename='.github/workflows/development-ci.yml',status='modified')],issue)

    def test_changed_contract_can_only_plan_full_execution_pending_independent_review(self):
        policy,issue,pull=issue_and_policy();proposed=copy.deepcopy(policy)
        issue['body']='Updated user-approved acceptance'
        proposed['qualification']['contracts']['84']['body_sha256']=hashlib.sha256(issue['body'].encode()).hexdigest()
        result=selection.build(shared,policy,'diff','faberline/core',HEAD,BASE,pull,[dict(filename='.github/issue-ci-required-gates.json',status='modified')],issue,proposed)
        self.assertEqual(result['qualification'],'not_evaluated')
        self.assertEqual(result['test_plan']['selected'],sorted(policy['qualification']['checks']))
        self.assertTrue(any('contract proposal' in reason for reason in result['pending_protected_review']))
        proposed['qualification']['contracts']['84']['applicability']['unit'].update(status='not_applicable',checks=[])
        with self.assertRaises(ValueError):
            selection.build(shared,policy,'diff','faberline/core',HEAD,BASE,pull,[dict(filename='.github/issue-ci-required-gates.json',status='modified')],issue,proposed)

    def test_legacy_gate_commands_and_each_case_definition_are_retained(self):
        prior=json.loads((ROOT/'scripts/pr-gates/tests/fixtures/pre-local-checks.json').read_text())
        self.assertEqual(CONFIG['qualification']['checks'],prior)
        for mode in ('diff','all'):
            policy,issue,pull=issue_and_policy()
            result=selection.build(shared,policy,mode,'faberline/core',HEAD,BASE,pull,[dict(filename='README.md',status='modified')],issue)
            self.assertEqual(result['test_plan']['checks'],prior)


if __name__=='__main__':unittest.main()
