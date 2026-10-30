"""Frozen impact planning retains full execution coverage."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

import test_plan

plan=test_plan.plan


class ImpactTests(unittest.TestCase):
    def test_rename_delete_and_spaces_keep_all_paths(self):
        rows=plan.parse_changes('R100\0crates/old/a.rs\0crates/new/a.rs\0D\0docs/old file.md\0')
        self.assertEqual(rows,[dict(status='R100',previous_filename='crates/old/a.rs',filename='crates/new/a.rs'),
                               dict(status='D',filename='docs/old file.md')])

    def test_partial_unknown_unsafe_and_duplicate_changes_refuse(self):
        for raw in ('M\0a.rs','R100\0old.rs\0','X\0a.rs\0','A\0../outside\0','M\0a.rs\0D\0a.rs\0'):
            with self.subTest(raw=raw),self.assertRaises(ValueError):plan.parse_changes(raw)

    def test_changed_leaf_expands_reverse_users_without_removing_other_tests(self):
        packages=[test_plan.package('leaf'),test_plan.package('user',['leaf']),test_plan.package('other')]
        with patch.object(plan.subprocess,'check_output',return_value='M\0crates/leaf/src/lib.rs\0') as run:
            result=plan.impact_plan('/candidate',packages,'a'*40,'b'*40)
        self.assertEqual(result['affected_packages'],['leaf','user'])
        self.assertEqual(result['execution_packages'],['leaf','other','user'])
        self.assertEqual(result['execution_selection'],'complete workspace')
        self.assertIn('b'*40+'...'+'a'*40,run.call_args.args[0])
        self.assertIn('--no-ext-diff',run.call_args.args[0])
        self.assertEqual(len(result['sha256']),64)

    def test_unknown_deleted_package_selects_full_coverage(self):
        packages=[test_plan.package('one'),test_plan.package('two')]
        with patch.object(plan.subprocess,'check_output',return_value='D\0crates/deleted/src/lib.rs\0'):
            result=plan.impact_plan('/candidate',packages,'a'*40,'b'*40)
        self.assertEqual(result['affected_packages'],['one','two'])


if __name__=='__main__':unittest.main()
