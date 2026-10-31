"""Actual staged workflow must preserve all ordinary and non-PR gates."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[3]
REF='d66014a9a8e3decbe3f534c43ab83bba305edbc4'
SPEC=importlib.util.spec_from_file_location('development_contract',ROOT/'scripts/issue-ci/development_pipeline.py')
d=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(d)
CONFIG=json.loads((ROOT/'.github/issue-ci-required-gates.json').read_text())
PATH=CONFIG['development_pipeline']['workflow']


class DevelopmentContractTests(unittest.TestCase):
    def test_actual_workflow_obeys_every_stage_barrier(self):
        result=d.workflow_contract((ROOT/PATH).read_text(),CONFIG)
        self.assertEqual(result['stages']['preflight'],['development-preflight'])
        self.assertEqual(result['stages']['impact'],['unit-plan'])

    def test_functional_and_performance_cannot_start_after_only_inventory(self):
        import yaml
        original=d.yaml_document((ROOT/PATH).read_text())
        for job in ('workspace-contracts','measured-performance'):
            candidate=copy.deepcopy(original);candidate['jobs'][job]['needs']='unit-plan'
            with self.subTest(job=job),self.assertRaisesRegex(RuntimeError,'earlier stage'):
                d.workflow_contract(yaml.safe_dump(candidate),CONFIG)

    def test_main_tags_and_manual_legacy_tests_keep_their_independent_trigger(self):
        legacy=d.yaml_document((ROOT/'.github/workflows/ci.yml').read_text())
        self.assertEqual(legacy['on']['push'],{'branches':['main'],'tags':['v*']})
        self.assertIn('workflow_dispatch',legacy['on'])
        self.assertNotIn('pull_request',legacy['on'])
        self.assertNotIn('needs',legacy['jobs']['crate-gates'])
        development=d.yaml_document((ROOT/PATH).read_text())
        self.assertEqual(development['on'],{'pull_request':{'branches':['main'],'types':['opened','reopened','synchronize','ready_for_review','edited']},'push':{'tags':['v*']}})

    def test_every_old_required_gate_is_kept_and_preflight_is_added(self):
        jobs=CONFIG['workflows'][0]['jobs']
        self.assertEqual(len(jobs),20)
        self.assertEqual(len(CONFIG['qualification']['checks']),145)
        self.assertIn('development preflight',jobs)
        self.assertEqual(sum(c['kind']=='unit' for c in CONFIG['qualification']['checks'].values()),70)
        self.assertEqual(sum(c['kind']=='functional_e2e' for c in CONFIG['qualification']['checks'].values()),62)
        self.assertEqual(sum(c['kind']=='performance' for c in CONFIG['qualification']['checks'].values()),1)
        legacy=json.loads((ROOT/'scripts/issue-ci/tests/fixtures/legacy-policy.json').read_text())
        labels={check.get('case_label',check['job']) for check in CONFIG['qualification']['checks'].values()}
        self.assertTrue(set(legacy['workflows'][0]['jobs']) <= labels)
        prior_checks=json.loads((ROOT/'scripts/pr-gates/tests/fixtures/pre-batch-checks.json').read_text())
        prior=dict(workflows=[dict(path='.github/workflows/ci.yml',jobs=[c['job']for c in prior_checks.values()])],
                   qualification=dict(checks=prior_checks))
        self.assertTrue(d.retains_workflows(prior,CONFIG))
        bad=copy.deepcopy(CONFIG);name=next(n for n,c in bad['qualification']['checks'].items()if c['kind']=='unit')
        del bad['qualification']['checks'][name]
        bad['development_pipeline']['grouped_checks'].remove(name)
        self.assertFalse(d.retains_workflows(prior,bad))

    def test_metadata_runtime_is_hash_locked_and_does_not_replace_product_commands(self):
        workflow=(ROOT/PATH).read_text()
        self.assertIn('--no-deps --only-binary=:all: --require-hashes',workflow)
        self.assertIn('python3 candidate/scripts/pr-gates/plan.py impact',workflow)
        self.assertIn('python3 candidate/scripts/pr-gates/batches.py run --group functional',workflow)
        self.assertIn('python3 candidate/scripts/pr-gates/measure.py',workflow)
        spec=importlib.util.spec_from_file_location('batch_commands',ROOT/'scripts/pr-gates/batches.py')
        batches=importlib.util.module_from_spec(spec);spec.loader.exec_module(batches)
        self.assertEqual(batches.RAFT['raft-full'],['cargo','test','--locked','-p','raft-core','-p','raft-runtime','--no-fail-fast'])
        self.assertIn('fetch-depth: 0',workflow)

    def test_each_producer_uses_the_same_reviewed_runtime_and_main_wake(self):
        self.assertEqual(CONFIG['aw_revision'],REF)
        writer=d.yaml_document((ROOT/'.github/workflows/issue-ci.yml').read_text())
        self.assertEqual(writer['on']['push'],{'branches':['main']})
        self.assertEqual(writer['on']['repository_dispatch'],{'types':['aw-development-reconcile']})
        self.assertEqual(writer['concurrency'],{'group':'issue-ci-handoff','cancel-in-progress':'false'})
        producer=CONFIG['development_reporter']
        raw=(ROOT/producer['path']).read_bytes()
        self.assertEqual(hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest(),producer['definition_sha'])
        self.assertIn(('@'+REF).encode(),raw)
        self.assertEqual(CONFIG['completion_owner'],'local')
        self.assertNotIn('native_auto_merge',CONFIG)
        self.assertNotIn('claude_final_review',CONFIG)
        self.assertIn('development_pipeline',CONFIG['ci_handoff_policy'])
        self.assertEqual(CONFIG['development_pipeline'],CONFIG['ci_handoff_policy']['development_pipeline'])

    def test_planner_failure_is_not_hidden_by_echo_and_matrix_has_direct_dependency(self):
        workflow=d.yaml_document((ROOT/PATH).read_text())
        for step in workflow['jobs']['unit-plan']['steps']:
            self.assertNotIn('echo "matrix=$(',step.get('run',''))
            self.assertNotIn('echo "result=$(',step.get('run',''))
        self.assertIn('unit-plan',workflow['jobs']['workspace-contracts']['needs'])
        self.assertNotIn('controller_source',CONFIG)


if __name__=='__main__':unittest.main()
