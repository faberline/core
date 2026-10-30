"""Actual staged workflow must preserve all ordinary and non-PR gates."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[3]
REF='8b8a5284ef928190c0d21718331de4b1732334c9'
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
        for job in ('crate-gates','source-issue-47','workspace-contracts','measured-performance'):
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
        self.assertEqual(development['on'],{'pull_request':{'branches':['main'],'types':['opened','reopened','synchronize','ready_for_review','edited']}})

    def test_every_old_required_gate_is_kept_and_preflight_is_added(self):
        jobs=CONFIG['workflows'][0]['jobs']
        self.assertEqual(len(jobs),145)
        self.assertIn('development preflight',jobs)
        self.assertEqual(sum(c['kind']=='unit' for c in CONFIG['qualification']['checks'].values()),70)
        self.assertEqual(sum(c['kind']=='functional_e2e' for c in CONFIG['qualification']['checks'].values()),62)
        self.assertEqual(sum(c['kind']=='performance' for c in CONFIG['qualification']['checks'].values()),1)
        legacy=json.loads((ROOT/'scripts/issue-ci/tests/fixtures/legacy-policy.json').read_text())
        self.assertTrue(set(legacy['workflows'][0]['jobs']) <= set(jobs))

    def test_metadata_runtime_is_hash_locked_and_does_not_replace_product_commands(self):
        workflow=(ROOT/PATH).read_text()
        self.assertIn('--no-deps --only-binary=:all: --require-hashes',workflow)
        self.assertIn('python3 candidate/scripts/pr-gates/plan.py impact',workflow)
        self.assertIn('python3 candidate/scripts/pr-gates/contracts.py run',workflow)
        self.assertIn('python3 candidate/scripts/pr-gates/measure.py',workflow)
        self.assertIn('cargo test --locked -p raft-core -p raft-runtime --no-fail-fast',workflow)
        self.assertIn('fetch-depth: 0',workflow)

    def test_each_producer_uses_the_same_reviewed_runtime_and_main_wake(self):
        self.assertEqual(CONFIG['aw_revision'],REF)
        writer=d.yaml_document((ROOT/'.github/workflows/issue-ci.yml').read_text())
        self.assertEqual(writer['on']['push'],{'branches':['main']})
        self.assertEqual(writer['on']['repository_dispatch'],{'types':['aw-verified-merge-reconcile']})
        self.assertEqual(writer['concurrency'],{'group':'issue-ci-handoff','cancel-in-progress':'false'})
        for producer in (CONFIG['native_auto_merge']['producer'],CONFIG['claude_final_review']['producer']):
            raw=(ROOT/producer['path']).read_bytes()
            self.assertEqual(hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest(),producer['definition_sha'])
            self.assertIn(('@'+REF).encode(),raw)
        self.assertIn('development_pipeline',CONFIG['ci_handoff_policy'])
        self.assertEqual(CONFIG['development_pipeline'],CONFIG['ci_handoff_policy']['development_pipeline'])


if __name__=='__main__':unittest.main()
