"""Documentation evidence is separate from mandatory nonzero unit evidence."""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

SPEC=importlib.util.spec_from_file_location('doc_runner',Path(__file__).parents[1]/'docs.py')
d=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(d)


def output(names=('example',),ignored=()):
    return (f'running {len(names)+len(ignored)} tests\n'+''.join(f'test {n} ... ok\n'for n in names)+''.join(f'test {n} ... ignored\n'for n in ignored)+f'test result: ok. {len(names)} passed; 0 failed; {len(ignored)} ignored; 0 measured; 0 filtered out; finished in 0.1s\n')


class DocumentationEvidenceTests(unittest.TestCase):
    def test_complete_named_roster_is_retained(self):
        result=d.evidence(output())
        self.assertEqual(result['harnesses'][0]['roster'],['example'])
        self.assertEqual(result['collected_examples'],1)
        self.assertFalse(result['counts_as_unit_evidence'])

    def test_real_empty_collection_is_explicit_and_never_unit_evidence(self):
        result=d.evidence(output(()))
        self.assertEqual(result['collected_examples'],0)
        self.assertFalse(result['counts_as_unit_evidence'])
        with self.assertRaisesRegex(ValueError,'zero-test'):
            d.unit.libtest_evidence(output(()),1)

    def test_missing_partial_filtered_ignored_or_duplicate_results_refuse(self):
        values=['','running 1 test\n',output().replace('0 filtered','1 filtered'),output((),('deferred',)),output(('a','a'))]
        for value in values:
            with self.subTest(value=value),self.assertRaises(ValueError):d.evidence(value)

    def test_multiple_rustdoc_harnesses_retain_all_examples(self):
        result=d.evidence(output(('one',))+output(())+output(('two',)))
        self.assertEqual(result['collected_examples'],2)
        self.assertEqual(len(result['harnesses']),3)

    def test_pending_ignored_roster_does_not_qualify_by_itself(self):
        result=d.evidence(output(('one',),('ignored',)),allow_ignored=True)
        self.assertEqual(result['harnesses'][0]['ignored_roster'],['ignored'])
        with self.assertRaises(ValueError):d.evidence(output(('one',),('ignored',)))

    def test_documentation_build_and_complete_profile_doctests_run(self):
        completed=subprocess.CompletedProcess([],0,stdout=output(),stderr='')
        with patch.object(d,'execute',return_value=(completed,{}))as execute,contextlib.redirect_stdout(io.StringIO()):
            d.run('/root','leaf','all-features',{})
        commands=[call.args[0]for call in execute.call_args_list]
        self.assertEqual(commands[0],['cargo','doc','--locked','-p','leaf','--all-features','--no-deps'])
        self.assertIn('--doc',commands[1]);self.assertIn('--all-features',commands[1]);self.assertNotIn('example',commands[1])

    def test_ignored_examples_require_complete_release_execution(self):
        pending=subprocess.CompletedProcess([],0,stdout=output(('one',),('ignored',)),stderr='')
        complete=subprocess.CompletedProcess([],0,stdout=output(('one','ignored')),stderr='')
        with patch.object(d,'execute',side_effect=[(complete,{}),(pending,{}),(complete,{})])as execute,contextlib.redirect_stdout(io.StringIO()):d.run('/root','leaf','default',{})
        self.assertIn('--release',execute.call_args_list[-1].args[0])
        self.assertIn('--include-ignored',execute.call_args_list[-1].args[0])

    def test_release_cannot_drop_pending_ignored_names(self):
        pending=subprocess.CompletedProcess([],0,stdout=output(('one',),('ignored',)),stderr='')
        incomplete=subprocess.CompletedProcess([],0,stdout=output(('one',)),stderr='')
        with patch.object(d,'execute',side_effect=[(incomplete,{}),(pending,{}),(incomplete,{})]),contextlib.redirect_stdout(io.StringIO()),self.assertRaisesRegex(ValueError,'did not all execute'):d.run('/root','leaf','default',{})

    def test_compiler_failure_cannot_be_hidden_by_successful_looking_stdout(self):
        failed=subprocess.CompletedProcess([],101,stdout=output(),stderr='compiler failed')
        with patch.object(d.subprocess,'run',return_value=failed),contextlib.redirect_stdout(io.StringIO()),contextlib.redirect_stderr(io.StringIO()),self.assertRaisesRegex(ValueError,'documentation command failed'):d.execute(['cargo','doc'],'/root')
