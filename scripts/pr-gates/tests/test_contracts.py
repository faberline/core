"""Contract selection and real libtest evidence controls; never run Cargo."""
import contextlib
import copy
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('contract_runner', Path(__file__).parents[1] / 'contracts.py')
c = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(c)


def package():
    return {'name': 'leaf', 'directory': 'crates/leaf', 'features': {'extra': []},
            'dependencies': [], 'targets': [{'name': 'leaf', 'kind': ['lib'],
                'required-features': [], 'harness': True, 'test': True}],
            'integration_targets': [{'name': 'it', 'kind': ['test'],
                'required-features': [], 'harness': True, 'test': True}]}


def output(names=('alpha',)):
    return (f'running {len(names)} tests\n' + ''.join(f'test {name} ... ok\n' for name in names)
            + f'test result: ok. {len(names)} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s\n')


class SelectionTests(unittest.TestCase):
    def test_every_manifest_feature_profile_is_selected(self):
        rows = c.matrix([package()])['include']
        self.assertEqual({r['profile'] for r in rows}, {'default', 'no-default', 'feature:extra', 'all-features'})
        self.assertEqual(len(rows), 4)

    def test_packages_without_integration_targets_do_not_invent_zero_test_jobs(self):
        empty = package();empty['name'] = 'unit_only';empty['integration_targets'] = []
        self.assertEqual(c.matrix([empty, package()]), c.matrix([package()]))

    def test_empty_inventory_refuses(self):
        for packages in [[], [dict(package(), integration_targets=[])]]:
            with self.assertRaisesRegex(ValueError, 'empty'): c.matrix(packages)

    def test_matrix_limit_refuses_without_truncation(self):
        with self.assertRaisesRegex(ValueError, 'oversized'):
            c.matrix([dict(package(), name='p' + str(i)) for i in range(65)])

    def test_profile_required_target_is_not_silently_dropped(self):
        p = package();p['integration_targets'][0]['required-features'] = ['extra']
        self.assertEqual({r['profile'] for r in c.matrix([p])['include']}, {'feature:extra', 'all-features'})

    def test_unreachable_target_refuses(self):
        p=package();p['integration_targets'][0]['required-features']=['absent']
        with self.assertRaisesRegex(ValueError, 'unreachable'):c.matrix([p])

    def test_unknown_profile_refuses(self):
        with self.assertRaisesRegex(ValueError, 'unknown'):c.active_targets(package(),'feature:absent')

    def test_target_and_profile_cannot_be_injected(self):
        p=package()
        with self.assertRaisesRegex(ValueError, 'outside'):c.command(p,'default',dict(p['integration_targets'][0],name='other'))

    def test_release_runs_include_ignored_and_never_filter(self):
        p=package();argv=c.command(p,'feature:extra',p['integration_targets'][0])
        self.assertEqual(argv[:8],['cargo','test','--release','--locked','-p','leaf','--test','it'])
        self.assertIn('--include-ignored',argv)
        self.assertEqual(argv[argv.index('--')+1:],['--include-ignored','--format','pretty','--color','never','--test-threads','1'])
        self.assertIn('--no-default-features',argv)

    def test_target_removal_refuses(self):
        old=package();new=copy.deepcopy(old);new['integration_targets']=[]
        with self.assertRaisesRegex(ValueError,'target removal'):c.compare_inventory([old],[new])

    def test_formerly_active_target_cannot_be_feature_gated_away(self):
        old=package();new=copy.deepcopy(old);new['integration_targets'][0]['required-features']=['extra']
        with self.assertRaisesRegex(ValueError,'enabled integration target lost'):c.compare_inventory([old],[new])

    def test_target_addition_does_not_reduce_existing_coverage(self):
        old=package();new=copy.deepcopy(old)
        new['integration_targets'].append(dict(new['integration_targets'][0],name='new'))
        c.compare_inventory([old],[new])
        self.assertEqual(len(c.active_targets(new,'default')),2)

    def test_baseline_feature_removal_still_refuses(self):
        old=package();new=copy.deepcopy(old);new['features']={}
        with self.assertRaisesRegex(ValueError,'feature removal'):c.compare_inventory([old],[new])


class EvidenceTests(unittest.TestCase):
    def run_fixture(self, stdout=output(), code=0, package_value=None, listings=None):
        log=io.StringIO();error=io.StringIO()
        if listings is None:
            listings = ['alpha: test\n\n1 test, 0 benchmarks\n'] * 10
        listings = iter(listings)
        def result(argv, **kwargs):
            if '--list' in argv:
                return subprocess.CompletedProcess(argv, 0, stdout=next(listings), stderr='')
            return subprocess.CompletedProcess(argv,code,stdout=stdout,stderr='raw error evidence')
        with patch.object(c.subprocess,'run',side_effect=result) as invoke:
            with contextlib.redirect_stdout(log),contextlib.redirect_stderr(error):
                try:c.run('/candidate',package_value or package(),'default',{'actual_head':'a'*40,'actual_base':'b'*40})
                except ValueError as exc:return log.getvalue(),error.getvalue(),invoke,exc
        return log.getvalue(),error.getvalue(),invoke,None

    def test_complete_evidence_retains_raw_output_and_digest(self):
        stdout,stderr,invoke,error=self.run_fixture()
        self.assertIsNone(error)
        self.assertIn(output(),stdout);self.assertIn('raw error evidence',stderr)
        record=json.loads(next(line.split(' ',1)[1] for line in stdout.splitlines()if line.startswith('CORE_CONTRACT_EVIDENCE_V1 ')))
        self.assertEqual(record['tests'][0]['roster'],['alpha'])
        self.assertEqual(record['discovered_roster'],['alpha'])
        self.assertEqual(len(record['stdout_sha256']),64)
        self.assertEqual(record['provenance']['actual_head'],'a'*40)
        self.assertNotIn('shell',invoke.call_args.kwargs)

    def test_nonzero_exit_never_passes_even_with_success_text(self):
        stdout,_,_,error=self.run_fixture(code=1)
        self.assertIsNotNone(error);self.assertIn('"result": "fail"',stdout)

    def test_missing_zero_ignored_filtered_or_duplicate_rosters_refuse(self):
        outputs=['',output(()),output().replace('0 ignored','1 ignored'),output().replace('0 filtered','1 filtered'),output(('alpha','alpha'))]
        for data in outputs:
            with self.subTest(data=data):self.assertIsNotNone(self.run_fixture(stdout=data)[3])

    def test_failure_does_not_hide_later_targets(self):
        p=package();p['integration_targets'].append(dict(p['integration_targets'][0],name='second'))
        _,_,invoke,error=self.run_fixture(code=1,package_value=p)
        self.assertIsNotNone(error);self.assertEqual(invoke.call_count,4)

    def test_omitted_or_changed_discovered_tests_do_not_pass(self):
        alpha='alpha: test\n\n1 test, 0 benchmarks\n'
        two='alpha: test\nbeta: test\n\n2 tests, 0 benchmarks\n'
        for listings in ([two,two], [alpha,two], [alpha,'']):
            with self.subTest(listings=listings):
                self.assertIsNotNone(self.run_fixture(listings=listings)[3])

    def test_discovery_failure_does_not_hide_later_targets(self):
        p=package();p['integration_targets'].append(dict(p['integration_targets'][0],name='second'))
        alpha='alpha: test\n\n1 test, 0 benchmarks\n'
        stdout,_,invoke,error=self.run_fixture(package_value=p,listings=['',alpha,alpha])
        self.assertIsNotNone(error)
        self.assertIn('"target": "second"', stdout)
        self.assertEqual(invoke.call_count,4)

    def test_no_targets_refuses_before_execution(self):
        p=package();p['integration_targets']=[]
        with patch.object(c.subprocess,'run')as invoke,self.assertRaisesRegex(ValueError,'empty'):
            c.run('/candidate',p,'default',{})
        invoke.assert_not_called()

    def test_identity_rechecked_immediately_before_product_execution(self):
        argv=['run','--root','/head','--baseline-root','/base','--expected-head','a'*40,'--expected-base','b'*40,'--package','leaf','--profile','default']
        with patch.object(c.unit,'verify_identity',return_value={})as identities,patch.object(c,'load',return_value=[package()]),patch.object(c,'run')as run:
            c.main(argv)
        self.assertEqual(identities.call_count,3);run.assert_called_once()

    def test_changed_checkout_after_contract_refuses_passing_result(self):
        argv=['run','--root','/head','--baseline-root','/base','--expected-head','a'*40,'--expected-base','b'*40,'--package','leaf','--profile','default']
        with patch.object(c.unit,'verify_identity',side_effect=[{},{},ValueError('checkout changed')]), \
             patch.object(c,'load',return_value=[package()]),patch.object(c,'run')as run:
            with self.assertRaisesRegex(ValueError,'checkout changed'):c.main(argv)
        run.assert_called_once()

    def test_failed_contract_still_rechecks_checkout(self):
        argv=['run','--root','/head','--baseline-root','/base','--expected-head','a'*40,'--expected-base','b'*40,'--package','leaf','--profile','default']
        with patch.object(c.unit,'verify_identity',return_value={})as identities, \
             patch.object(c,'load',return_value=[package()]),patch.object(c,'run',side_effect=ValueError('product failed')):
            with self.assertRaisesRegex(ValueError,'product failed'):c.main(argv)
        self.assertEqual(identities.call_count,3)

    def test_invalid_row_refuses_before_product_execution(self):
        argv=['run','--root','/head','--baseline-root','/base','--expected-head','a'*40,'--expected-base','b'*40,'--package','other','--profile','default']
        with patch.object(c.unit,'verify_identity',return_value={}),patch.object(c,'load',return_value=[package()]),patch.object(c,'run')as run,contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):c.main(argv)
        run.assert_not_called()
