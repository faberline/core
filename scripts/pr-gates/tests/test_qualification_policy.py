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
CONFIG=json.loads((ROOT/'.github/issue-ci-required-gates.json').read_text())
HEAD,BASE='a'*40,'b'*40
WORKFLOW='.github/workflows/ci.yml'


def pull():return dict(number=75,title='Synthetic',body='Closes #73',changed_files=1,head=dict(sha=HEAD,repo={'full_name':'o/r'}),base=dict(sha=BASE,repo={'full_name':'o/r'}))
def files(path='crates/raft-runtime/src/application/host.rs'):return [dict(filename=path,status='modified')]
def scope(config=None,path='crates/raft-runtime/src/application/host.rs'):
    cfg=copy.deepcopy(config or CONFIG)
    blob=cfg['qualification']['trusted_workflows'][WORKFLOW]['definition_sha']
    issue=dict(number=73,state='open',title='Synthetic issue',body='Synthetic complete contract',labels=[],url='https://api.github.com/repos/o/r/issues/73',repository_url='https://api.github.com/repos/o/r')
    case=cfg['qualification']['contracts']['73']
    for name in ('title','body'):case[name+'_sha256']=hashlib.sha256(issue[name].encode()).hexdigest()
    return q.scoped_config(cfg,pull(),files(path),{(WORKFLOW,ref):dict(sha=blob)for ref in [HEAD,BASE]},issue)



class FullPolicyTests(unittest.TestCase):
    def test_issue_47_explicit_source_gates_are_always_required(self):
        expected = {
            'policy-8': 'candidate issue 47 (proposal)',
            'policy-9': 'candidate issue 47 (raft-full)',
            'policy-10': 'candidate issue 47 (clippy)',
        }
        for key, job in expected.items():
            self.assertIn(key, CONFIG['qualification']['always'])
            self.assertEqual(CONFIG['qualification']['checks'][key]['job'], job)
            self.assertIn(job, CONFIG['workflows'][0]['jobs'])
        workflow = (ROOT / WORKFLOW).read_text()
        self.assertIn('cargo test --locked -p raft-core -p raft-runtime --no-fail-fast', workflow)
        self.assertIn('cargo clippy --locked -p raft-runtime --all-targets -- -D warnings', workflow)
        self.assertIn('python3 scripts/pr-gates/proposal_gate.py', workflow)

    def test_registry_prefetch_does_not_write_a_lockfile_into_candidate(self):
        workflow=(ROOT/WORKFLOW).read_text()
        self.assertIn('cp -R candidate/scripts/pr-gates/generated-rust-dependencies "$RUNNER_TEMP/generated-rust-dependencies"',workflow)
        self.assertIn('cargo fetch --manifest-path "$RUNNER_TEMP/generated-rust-dependencies/Cargo.toml"',workflow)
        self.assertNotIn('cargo fetch --manifest-path candidate/scripts/pr-gates/generated-rust-dependencies/Cargo.toml',workflow)

    def test_review_and_writer_are_bound_to_reviewed_todo_source(self):
        self.assertEqual(CONFIG['aw_revision'],'8e2e93e5ba8feb30b6668de39139980ea3bb085c')
        for producer in [CONFIG['claude_final_review']['producer'],CONFIG['native_auto_merge']['producer']]:
            raw=(ROOT/producer['path']).read_bytes()
            blob=hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()
            self.assertEqual(blob,producer['definition_sha'])
            self.assertIn(b'@8e2e93e5ba8feb30b6668de39139980ea3bb085c',raw)
        self.assertEqual(CONFIG['native_auto_merge']['status_context'],'claude/verified-merge')
        self.assertEqual(CONFIG['native_auto_merge']['status_app_id'],15368)

    def test_copied_inspection_modules_match_published_shared_source(self):
        expected={'handoff.py':'b69f57781585cd26a5372ab132e4cfe46065ee3a262ef79b25494670e82d4438',
                  'qualification.py':'f5dbf2f07cd626d449a47938ba045bf61e3197cfa4b18d342e6fc6710e242ceb',
                  'json_data.py':'35dc74be907733cfff108eae9bf217eec5c69a028487919f84c86c06d805facf'}
        for name,digest in expected.items():
            self.assertEqual(hashlib.sha256((ROOT/'scripts/issue-ci'/name).read_bytes()).hexdigest(),digest)

    def test_operational_root_requires_complete_full_contract(self):
        spec=importlib.util.spec_from_file_location('policy_guard',ROOT/'scripts/issue-ci/check-policy.py')
        guard=importlib.util.module_from_spec(spec);spec.loader.exec_module(guard)
        self.assertEqual(guard.check(ROOT/'.github/issue-ci-required-gates.json')['lifecycle_mode'],'full')
        import tempfile
        with tempfile.TemporaryDirectory()as directory:
            path=Path(directory)/'policy.json'
            for edit in [lambda c:c.pop('qualification'),lambda c:c.update(qualification_blockers=['unknown']),lambda c:c.update(_qualification_context={})]:
                candidate=copy.deepcopy(CONFIG);edit(candidate);path.write_text(json.dumps(candidate))
                with self.assertRaises(RuntimeError):guard.check(path)

    def test_actual_workflow_definition_is_hash_bound(self):
        raw=(ROOT/WORKFLOW).read_bytes()
        blob=hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()
        self.assertEqual(blob,CONFIG['qualification']['trusted_workflows'][WORKFLOW]['definition_sha'])
        self.assertEqual(blob,CONFIG['claude_final_review']['gates'][WORKFLOW]['definition_sha'])

    def test_full_policy_does_not_silently_accept_old_five_jobs(self):
        policy=q.validate(CONFIG)
        counts={kind:sum(c['kind']==kind for c in policy['checks'].values())for kind in ['unit','functional_e2e','performance','policy']}
        self.assertEqual(counts,dict(unit=70,functional_e2e=62,performance=1,policy=12))
        self.assertEqual(len(CONFIG['workflows'][0]['jobs']),145)
        self.assertEqual(len(set(CONFIG['workflows'][0]['jobs'])),145)
        self.assertIn('unit-stage',policy['always'])
        self.assertEqual(policy['checks']['unit-stage']['job'],'candidate unit/static complete')

    def test_complete_workspace_is_required_even_for_docs_and_unknown_paths(self):
        expected=set(CONFIG['workflows'][0]['jobs'])
        for path in ['README.md','docs/guide.md','new/unknown.rs','crates/raft-runtime/src/application/host.rs']:
            with self.subTest(path=path):
                scoped=scope(path=path)
                self.assertEqual(scoped['_qualification_context']['issues'],[])
                self.assertEqual(set(scoped['workflows'][0]['jobs']),expected)
                self.assertTrue(all(not value for value in scoped['_qualification_context']['exemptions'].values()))

    def test_workflow_driver_policy_or_manifest_change_cannot_self_approve(self):
        for path in ['.github/workflows/ci.yml','.github/issue-ci-required-gates.json','scripts/pr-gates/plan.py','Cargo.toml','crates/raft-runtime/Cargo.toml']:
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

    def test_real_issue_registry_keeps_unreviewed_requirements_on_hold(self):
        policy=q.validate(CONFIG)
        self.assertEqual(policy['version'],2)
        self.assertEqual(set(policy['contracts']['73']['applicability']),q.CATEGORIES)
        for number in ('15','45'):
            case=policy['contracts'][number]
            self.assertTrue(case['unresolved'])
            self.assertTrue(all(value['status']=='unknown' for value in case['applicability'].values()))
        pull_with_missing=pull();pull_with_missing['body']='Closes #999999'
        self.assertIn('linked issue has no reviewed contract',q.select(CONFIG,pull_with_missing,files())['issues'])

    def test_checkout_issue_keeps_every_existing_framework_gate(self):
        policy=q.validate(CONFIG)
        previous=policy['contracts']['76'];current=policy['contracts']['78']
        self.assertEqual(current['title_sha256'],'3a5136fe711e785f7207b301b354ea5e99c9636df12911b81d993053c7ee1838')
        self.assertEqual(current['body_sha256'],'f310be79e35a53855ab791d2eda2f16301a2110c746ee5f8e4b16b28c634cf35')
        for category in q.CATEGORIES:
            self.assertEqual(current['applicability'][category]['status'],'required')
            self.assertEqual(current['applicability'][category]['checks'],previous['applicability'][category]['checks'])
        self.assertEqual(current['additional'],[])
        self.assertEqual(current['unresolved'],[])
        self.assertEqual(CONFIG['qualification'],CONFIG['ci_handoff_policy']['qualification'])

    def test_sequence_issue_requires_existing_product_coverage(self):
        policy=q.validate(CONFIG)
        previous=policy['contracts']['78'];current=policy['contracts']['80']
        for category in q.CATEGORIES:
            self.assertEqual(current['applicability'][category]['status'],'required')
            self.assertEqual(current['applicability'][category]['checks'],
                             previous['applicability'][category]['checks'])
        self.assertEqual(current['unresolved'],[])
        self.assertEqual(current['additional'],[])
        self.assertEqual(CONFIG['qualification'],CONFIG['ci_handoff_policy']['qualification'])

    def test_candidate_inspection_job_does_not_grant_policy_approval(self):
        raw=(ROOT/WORKFLOW).read_text()
        self.assertIn('name: candidate source issue contract inspection',raw)
        self.assertIn('faberline/aw/.github/actions/issue-contract@8e2e93e5ba8feb30b6668de39139980ea3bb085c',raw)
        self.assertIn('gate infrastructure change requires reviewed policy update',
                      ' '.join(scope(path='.github/workflows/ci.yml')['_qualification_context']['issues']))

    def test_zero_check_unapproved_or_malformed_exemption_refuses(self):
        cfg=copy.deepcopy(CONFIG);cfg['qualification']['always']=[]
        with self.assertRaises(RuntimeError):q.validate(cfg)
        cfg=copy.deepcopy(CONFIG);cfg['qualification']['rules'][0]['exempt']={'performance':''}
        with self.assertRaises(RuntimeError):q.validate(cfg)
