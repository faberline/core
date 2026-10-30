"""Validate the deployed completion boundary, not actor audit or quiescence."""
import hashlib
import json
from pathlib import Path
import unittest

import test_development_pipeline_contract as development

ROOT=Path(__file__).resolve().parents[3]
CONFIG=json.loads((ROOT/'.github/issue-ci-required-gates.json').read_text())


class CompletionBoundaryTests(unittest.TestCase):
    def test_drain_is_inactive_and_cannot_supply_guard_activation(self):
        marker=CONFIG.get('issue_guard_cutover')
        if marker is not None:
            self.assertEqual(set(marker),{'version','phase','nonce'})
            self.assertEqual((marker['version'],marker['phase']),(1,'drain'))
            self.assertRegex(marker['nonce'],r'^[0-9a-f]{32}$')
            self.assertNotIn('issue_guard',CONFIG)
            self.assertNotIn('issue_guard_rollout',CONFIG)
            self.assertIs(CONFIG['issue_recovery']['enabled'],False)
        else:
            # A later activation still needs the shared live verifier. Static
            # source assertions alone cannot authenticate its operator grant.
            self.assertIs(CONFIG['issue_guard']['enabled'],True)
            self.assertEqual(set(CONFIG['issue_guard_rollout']),{'version','source','epoch'})
            self.assertRegex(CONFIG['issue_guard_rollout']['source'],r'^[0-9a-f]{40}$')
            self.assertIs(CONFIG['issue_recovery']['enabled'],True)

    def test_recovery_is_pinned_schedule_free_and_serialized_with_the_writer(self):
        value=CONFIG['issue_recovery'];spec=value['producer']
        self.assertEqual(set(value),{'version','enabled','producer'})
        self.assertEqual(value['version'],2)
        self.assertEqual((spec['path'],spec['id'],spec['job']),
                         ('.github/workflows/issue-recovery.yml',402173927,'recovery'))
        raw=(ROOT/spec['path']).read_bytes()
        self.assertEqual(spec['definition_sha'],hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest())
        recovery=development.d.yaml_document(raw.decode())
        self.assertEqual(recovery['on'],dict(repository_dispatch=dict(types=['aw-verified-merge-recover']),
            workflow_run=dict(workflows=['Issue CI and Claude outcome'],types=['completed'],branches=['main'])))
        self.assertEqual(recovery['permissions'],{})
        writer=development.d.yaml_document((ROOT/'.github/workflows/issue-ci.yml').read_text())
        self.assertEqual(recovery['concurrency'],writer['concurrency'])
        self.assertEqual(set(recovery['jobs']),{'recovery'})
        job=recovery['jobs']['recovery']
        self.assertEqual(job['steps'],[dict(uses='faberline/aw/.github/actions/verified-issue-recovery@'+CONFIG['aw_revision'])])
        self.assertEqual(job['permissions'],dict(actions='read',contents='write',issues='write',
                                              **{'pull-requests':'read','statuses':'write'}))

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
