"""Proposed full policy must retain every row and refuse untrusted evidence."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[3]
SPEC=importlib.util.spec_from_file_location('qualification_policy',ROOT/'scripts/issue-ci/qualification.py')
q=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(q)
CONFIG=json.loads((ROOT/'.github/core-coverage-policy.json').read_text())
HEAD,BASE='a'*40,'b'*40
WORKFLOW='.github/workflows/ci.yml'


def pull():return dict(number=75,changed_files=1,head=dict(sha=HEAD),base=dict(sha=BASE))
def files(path='crates/raft-runtime/src/application/host.rs'):return [dict(filename=path,status='modified')]
def scope(config=None,path='crates/raft-runtime/src/application/host.rs'):
    cfg=copy.deepcopy(config or CONFIG)
    blob=cfg['qualification']['trusted_workflows'][WORKFLOW]['definition_sha']
    return q.scoped_config(cfg,pull(),files(path),{(WORKFLOW,ref):dict(sha=blob)for ref in [HEAD,BASE]})


class FullPolicyTests(unittest.TestCase):
    def test_actual_workflow_definition_is_hash_bound(self):
        raw=(ROOT/WORKFLOW).read_bytes()
        blob=hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()
        self.assertEqual(blob,CONFIG['qualification']['trusted_workflows'][WORKFLOW]['definition_sha'])
        self.assertEqual(blob,CONFIG['claude_final_review']['gates'][WORKFLOW]['definition_sha'])

    def test_full_policy_does_not_silently_accept_old_five_jobs(self):
        policy=q.validate(CONFIG)
        counts={kind:sum(c['kind']==kind for c in policy['checks'].values())for kind in ['unit','functional_e2e','performance','policy']}
        self.assertEqual(counts,dict(unit=70,functional_e2e=62,performance=1,policy=7))
        self.assertEqual(len(CONFIG['workflows'][0]['jobs']),140)
        self.assertEqual(len(set(CONFIG['workflows'][0]['jobs'])),140)

    def test_complete_workspace_is_required_even_for_docs_and_unknown_paths(self):
        expected=set(CONFIG['workflows'][0]['jobs'])
        for path in ['README.md','docs/guide.md','new/unknown.rs','crates/raft-runtime/src/application/host.rs']:
            with self.subTest(path=path):
                scoped=scope(path=path)
                self.assertEqual(scoped['_qualification_context']['issues'],[])
                self.assertEqual(set(scoped['workflows'][0]['jobs']),expected)
                self.assertTrue(all(not value for value in scoped['_qualification_context']['exemptions'].values()))

    def test_workflow_driver_policy_or_manifest_change_cannot_self_approve(self):
        for path in ['.github/workflows/ci.yml','.github/issue-ci-required-gates.json','.github/core-coverage-policy.json','scripts/pr-gates/plan.py','Cargo.toml','crates/raft-runtime/Cargo.toml']:
            with self.subTest(path=path):self.assertTrue(scope(path=path)['_qualification_context']['issues'])

    def test_missing_base_or_capped_or_partial_files_refuse(self):
        for count,rows in [(0,[]),(2,files()),(3000,files())]:
            p=pull();p['changed_files']=count
            result=q.select(CONFIG,p,rows)
            self.assertTrue(result['issues'])
        p=pull();p['base']={}
        self.assertTrue(q.select(CONFIG,p,files())['issues'])

    def test_head_or_base_workflow_change_is_untrusted(self):
        for bad in [HEAD,BASE]:
            blob=CONFIG['qualification']['trusted_workflows'][WORKFLOW]['definition_sha']
            definitions={(WORKFLOW,ref):dict(sha=blob)for ref in [HEAD,BASE]}
            definitions[(WORKFLOW,bad)]['sha']='0'*40
            selected=q.scoped_config(CONFIG,pull(),files(),definitions)
            self.assertEqual(selected['_qualification_context']['untrusted_workflows'],[WORKFLOW])

    def test_run_requires_exact_pr_head_base_id_and_attempt(self):
        cfg=scope();run=dict(path=WORKFLOW,workflow_id=384237875,run_attempt=1,pull_requests=[dict(number=75,head=dict(sha=HEAD),base=dict(sha=BASE))])
        self.assertTrue(q.trusted_run(run,cfg))
        for change in [dict(workflow_id=7),dict(run_attempt=0),dict(pull_requests=[]),dict(pull_requests=[dict(number=75,head=dict(sha='0'*40),base=dict(sha=BASE))]),dict(pull_requests=[dict(number=75,head=dict(sha=HEAD),base=dict(sha='0'*40))])]:
            self.assertFalse(q.trusted_run(dict(run,**change),cfg))

    def test_ci_handoff_and_merge_use_the_same_complete_semantics(self):
        for key in ['workflows','lifecycle_mode','qualification_blockers','qualification']:
            self.assertEqual(CONFIG[key],CONFIG['ci_handoff_policy'][key])

    def test_zero_check_unapproved_or_malformed_exemption_refuses(self):
        cfg=copy.deepcopy(CONFIG);cfg['qualification']['always']=[]
        with self.assertRaises(RuntimeError):q.validate(cfg)
        cfg=copy.deepcopy(CONFIG);cfg['qualification']['rules'][0]['exempt']={'performance':''}
        with self.assertRaises(RuntimeError):q.validate(cfg)
