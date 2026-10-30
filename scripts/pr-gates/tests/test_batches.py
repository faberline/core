"""Batches must preserve every case and fail on partial or failed execution."""
import contextlib
import copy
import importlib.util
import io
import json
from pathlib import Path
import re
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[3]
SPEC=importlib.util.spec_from_file_location('core_batches',ROOT/'scripts/pr-gates/batches.py')
b=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(b)
POLICY=json.loads((ROOT/'.github/issue-ci-required-gates.json').read_text())
SETTINGS=json.loads((ROOT/'.github/development-test-batches.json').read_text())
BEFORE=json.loads((ROOT/'scripts/pr-gates/tests/fixtures/pre-batch-checks.json').read_text())


def rows():
    result={'unit':[], 'functional':[]}
    for check in BEFORE.values():
        if check['kind'] in ('unit','functional_e2e'):
            match=re.fullmatch(r'candidate (unit-static|contracts) \(([^,]+), (.+)\)',check['job'])
            result['unit' if check['kind']=='unit' else 'functional'].append(dict(package=match[2],profile=match[3]))
    return result


def plan(policy=None):
    source=rows()
    return b.build(source['unit'],source['functional'],policy or POLICY,SETTINGS)


class BatchPlanningTests(unittest.TestCase):
    def test_all_145_prior_cases_and_kinds_survive_20_physical_jobs(self):
        self.assertEqual(set(BEFORE)|{'policy-11'},set(POLICY['qualification']['checks']))
        self.assertEqual(len(BEFORE),144)
        for name,old in BEFORE.items():
            check=POLICY['qualification']['checks'][name]
            self.assertEqual(old['kind'],check['kind'])
            self.assertEqual(old['job'],check.get('case_label',check['job']))
        result=plan()
        self.assertEqual([len(result['groups'][g])for g in ('unit','functional')],[8,8])
        self.assertEqual([sum(len(x['cases'])for x in result['groups'][g])for g in ('unit','functional')],[70,70])
        self.assertEqual(len(POLICY['workflows'][0]['jobs']),20)
        self.assertEqual(len({c['id']for batches in result['groups'].values()for batch in batches for c in batch['cases']}),140)

    def test_fresh_matrix_cannot_drop_duplicate_or_add_a_feature_row(self):
        for group in ('unit','functional'):
            for change in ('drop','duplicate','add'):
                source=rows()
                if change=='drop':source[group].pop()
                if change=='duplicate':source[group].append(source[group][0])
                if change=='add':source[group].append(dict(package='new',profile='default'))
                with self.subTest(group=group,change=change),self.assertRaisesRegex(ValueError,'roster'):
                    b.build(source['unit'],source['functional'],POLICY,SETTINGS)

    def test_case_mapping_category_and_batch_mismatch_refuse(self):
        for change in ('id','kind','batch','registry'):
            policy=copy.deepcopy(POLICY);name=next(k for k,v in policy['qualification']['checks'].items()if v['kind']=='unit')
            check=policy['qualification']['checks'][name]
            if change=='id':check['case_id']='other'
            if change=='kind':check['kind']='performance'
            if change=='batch':check['job']='other'
            if change=='registry':policy['development_pipeline']['grouped_checks'].remove(name)
            with self.subTest(change=change),self.assertRaises(ValueError):plan(policy)

    def test_batch_counts_belong_to_core_and_invalid_empty_counts_refuse(self):
        for count in (0,True,33,'8'):
            value=dict(SETTINGS,unit_batches=count)
            with self.subTest(count=count),self.assertRaises(ValueError):b.count_config(value)
        self.assertEqual(b.count_config(dict(version=1,unit_batches=3,functional_batches=4))['unit_batches'],3)
        with self.assertRaises(ValueError):b.shards([dict(package='one',id='one')],2)

    def test_layout_is_deterministic_and_unit_commands_do_not_unify_features(self):
        source=rows();first=plan();source['unit'].reverse();source['functional'].reverse()
        self.assertEqual(first,b.build(source['unit'],source['functional'],POLICY,SETTINGS))
        case=next(c for batch in first['groups']['unit']for c in batch['cases']if c['profile'].startswith('feature:'))
        commands=b.commands(case,Path('/candidate'),Path('/base'),'a'*40,'b'*40)
        self.assertEqual(len(commands),3)
        self.assertEqual([cmd[-1]for cmd in commands[:2]],['unit','clippy'])
        self.assertIn(case['profile'],commands[2])
        self.assertTrue(commands[2][1].endswith('/docs.py'))


class BatchExecutionTests(unittest.TestCase):
    def fixture(self, cases):
        return dict(sha256='d'*64,groups={'unit':[dict(batch='01/01',cases=cases)]})

    def cases(self):
        return [dict(id=n,label=n,runner='unit',package=n,profile='default')for n in ('a','b')]

    def execute(self, value=None, invoke=None, group='unit'):
        value=value or self.fixture(self.cases());calls=[]
        if invoke is None:
            def invoke(argv,**kwargs):calls.append((argv,kwargs));return SimpleNamespace(returncode=0)
        with patch.object(b.unit,'verify_identity',return_value={}),contextlib.redirect_stdout(io.StringIO())as log:
            result=b.execute(value,group,'01/01',Path('/candidate'),Path('/base'),'a'*40,'b'*40,invoke)
        return result,calls,log.getvalue()

    def test_every_unit_case_runs_unit_clippy_docs_and_retains_evidence(self):
        result,calls,log=self.execute()
        self.assertEqual(len(calls),6)
        self.assertEqual(result['expected'],['a','b'])
        self.assertTrue(all(r['result']=='pass'for r in result['results']))
        self.assertEqual(log.count('CORE_BATCH_CASE_V1 '),2)
        self.assertIn('CORE_BATCH_EVIDENCE_V1 ',log)

    def test_any_case_failure_keeps_all_later_commands_and_fails_whole_job(self):
        calls=[]
        def invoke(argv,**kwargs):calls.append(argv);return SimpleNamespace(returncode=1 if len(calls)==1 else 0)
        with self.assertRaisesRegex(ValueError,'cases failed'):self.execute(invoke=invoke)
        self.assertEqual(len(calls),6)

    def test_zero_unknown_and_duplicate_batch_cases_never_pass(self):
        for cases in ([],self.cases()*2):
            with self.subTest(cases=cases),self.assertRaises(ValueError):
                self.execute(self.fixture(cases))
        with self.assertRaisesRegex(ValueError,'unknown'):
            self.execute(dict(sha256='d'*64,groups={'unit':[]}))

    def test_nonzero_or_missing_original_unit_roster_still_fails_original_driver(self):
        with self.assertRaises(ValueError):b.unit.libtest_evidence('running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0s\n',1)
        with self.assertRaises(ValueError):b.unit.libtest_evidence('',1)

    def test_each_live_contract_case_gets_fresh_kind_and_cleanup_failure_fails(self):
        import tempfile
        cases=[dict(id=str(i),label=str(i),runner='contracts',package='service-k8s',profile=p)for i,p in enumerate(('default','all-features'))]
        calls=[]
        def invoke(argv,**kwargs):calls.append(argv);return SimpleNamespace(returncode=0)
        with tempfile.TemporaryDirectory()as directory,patch.object(b.unit,'verify_identity',return_value={}),patch.object(b.subprocess,'check_output',return_value='kind-core-contracts\n'),contextlib.redirect_stdout(io.StringIO()):
            value=dict(sha256='d'*64,groups={'functional':[dict(batch='01/01',cases=cases)]})
            b.execute(value,'functional','01/01',Path(directory),Path('/base'),'a'*40,'b'*40,invoke)
            self.assertEqual(sum('create'in c for c in calls),2)
            self.assertEqual(sum('delete'in c for c in calls),2)
            def failing(argv,**kwargs):return SimpleNamespace(returncode=1 if 'delete'in argv else 0)
            with self.assertRaisesRegex(ValueError,'cases failed'):
                b.execute(value,'functional','01/01',Path(directory),Path('/base'),'a'*40,'b'*40,failing)

    def test_legacy_command_does_not_inherit_live_cluster_flag(self):
        case=dict(id='legacy',label='raft-core',runner='legacy',package='raft-core',command=['cargo','test','--locked','-p','raft-core'])
        with patch.dict(b.os.environ,{'SERVICE_K8S_LIVE_KUBE':'1'}):result,calls,_=self.execute(self.fixture([case]))
        self.assertNotIn('SERVICE_K8S_LIVE_KUBE',calls[0][1]['env'])


if __name__=='__main__':unittest.main()
