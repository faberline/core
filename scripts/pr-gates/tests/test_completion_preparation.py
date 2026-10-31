"""Validate the deployed completion boundary, not actor audit or quiescence."""
import hashlib
import json
from pathlib import Path
import unittest

import test_development_pipeline_contract as development

ROOT=Path(__file__).resolve().parents[3]
CONFIG=json.loads((ROOT/'.github/issue-ci-required-gates.json').read_text())


class CompletionBoundaryTests(unittest.TestCase):
    def test_local_completion_has_no_cloud_review_merge_or_recovery_producer(self):
        self.assertEqual(CONFIG['completion_owner'],'local')
        for field in ('claude_final_review','native_auto_merge','issue_recovery','issue_guard_cutover','issue_guard_rollout'):
            self.assertNotIn(field,CONFIG)
        for file in ('claude-final-review.yml','issue-recovery.yml'):
            self.assertFalse((ROOT/'.github/workflows'/file).exists())

    def test_only_label_writer_is_pinned_and_has_no_completion_permissions(self):
        spec=CONFIG['development_reporter']
        self.assertEqual((spec['path'],spec['id'],spec['job']),
                         ('.github/workflows/issue-ci.yml',394215097,'report'))
        raw=(ROOT/spec['path']).read_bytes()
        self.assertEqual(spec['definition_sha'],hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest())
        writer=development.d.yaml_document(raw.decode())
        self.assertEqual(writer['permissions'],{})
        self.assertNotIn('schedule',writer['on'])
        self.assertEqual(writer['concurrency'],{'group':'issue-ci-handoff','cancel-in-progress':'false'})
        self.assertEqual(set(writer['jobs']),{'report'})
        job=writer['jobs']['report']
        self.assertEqual(job['steps'],[{'uses':'faberline/aw/.github/actions/issue-ci@'+CONFIG['aw_revision'],
                                       'with':{'operation':'development-report'}}])
        self.assertEqual(job['permissions'],dict(actions='read',contents='read',issues='write',**{'pull-requests':'read'}))

    def test_manifest_keeps_worker_families_and_each_single_writer_audit_target(self):
        manifest=json.loads((ROOT/'.github/issue-guard-actors.json').read_text())
        self.assertEqual(set(manifest),{'version','actors'})
        self.assertEqual(manifest['version'],1)
        actors=manifest['actors'];names=[a['id']for a in actors]
        self.assertEqual(len(names),len(set(names)))
        self.assertTrue(all(set(a)=={'id','role'}for a in actors))
        for name in ('claude-core-project-issue-work','agy-core-issue-work','codex-pipeline-controller'):
            self.assertIn(name,names)
        self.assertEqual(sum(a['role']=='writer'for a in actors),1)
        self.assertEqual(sum(a['role']=='recovery'for a in actors),1)
        self.assertGreaterEqual(sum(a['role']=='worker'for a in actors),1)
        self.assertNotIn('audit',manifest)
