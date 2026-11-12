#!/usr/bin/env python3
"""Tests for ci/check-release-policy.py (issue #226, part 3 of #223).

No network. The real files are read from the repository root.
"""

import copy
import importlib.util
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_spec = importlib.util.spec_from_file_location(
    "crp", Path(__file__).with_name("check-release-policy.py")
)
crp = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(crp)

REL = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
CI = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
EXP = crp.expected_jobs(REL, CI)
POLICY = json.loads((ROOT / ".github/aw-release-gates.json").read_text(encoding="utf-8"))
BLOB = POLICY["workflows"][0]["definition_sha"]
CONTRACT = json.loads((ROOT / ".github/aw-release.json").read_text(encoding="utf-8"))
DOC = (ROOT / "docs/operations/release-pipeline.md").read_text(encoding="utf-8")


class RealFiles(unittest.TestCase):
    def test_real_files_pass(self):
        self.assertEqual(crp.check_policy(POLICY, EXP, BLOB), [])
        self.assertEqual(crp.check_contract(CONTRACT, set(EXP)), [])
        self.assertEqual(crp.check_doc(DOC), [])


class ExpandNames(unittest.TestCase):
    def test_expand_names_examples(self):
        self.assertEqual(EXP["preflight"], "policy")
        self.assertEqual(
            EXP["ci / test: feature rows (index-text)"], "unit"
        )
        self.assertEqual(EXP["ci / e2e: service-k8s kind (default)"], "functional_e2e")
        self.assertEqual(EXP["perf: thresholds"], "performance")
        self.assertEqual(EXP["gate"], "policy")
        names = crp.expand_names(REL, CI)
        self.assertEqual(len(names), len(set(names)))

    def test_unknown_uses_fails(self):
        bad = REL.replace("uses: ./.github/workflows/ci.yml", "uses: other/workflow.yml", 1)
        with self.assertRaises(ValueError):
            crp.expand_names(bad, CI)

    def test_unknown_kind_fails(self):
        with self.assertRaises(ValueError):
            crp.kind_for("something else")


class PolicyChecks(unittest.TestCase):
    def policy_with(self, jobs=None):
        p = copy.deepcopy(POLICY)
        if jobs is not None:
            p["workflows"][0]["jobs"] = jobs
        return p

    def test_missing_job_fails(self):
        jobs = copy.deepcopy(POLICY["workflows"][0]["jobs"])
        del jobs["perf: thresholds"]
        self.assertNotEqual(crp.check_policy(self.policy_with(jobs), EXP, BLOB), [])

    def test_extra_job_fails(self):
        jobs = copy.deepcopy(POLICY["workflows"][0]["jobs"])
        jobs["ci / test: not real"] = "unit"
        self.assertNotEqual(crp.check_policy(self.policy_with(jobs), EXP, BLOB), [])

    def test_wrong_kind_fails(self):
        jobs = copy.deepcopy(POLICY["workflows"][0]["jobs"])
        jobs["perf: thresholds"] = "unit"
        self.assertNotEqual(crp.check_policy(self.policy_with(jobs), EXP, BLOB), [])

    def test_kinds_must_all_appear(self):
        jobs = copy.deepcopy(POLICY["workflows"][0]["jobs"])
        del jobs["perf: thresholds"]
        exp = copy.deepcopy(EXP)
        del exp["perf: thresholds"]
        self.assertNotEqual(crp.check_policy(self.policy_with(jobs), exp, BLOB), [])

    def test_wrong_blob_sha_fails(self):
        self.assertNotEqual(crp.check_policy(POLICY, EXP, "0" * 40), [])

    def test_bool_id_fails(self):
        p = self.policy_with()
        p["workflows"][0]["id"] = True
        self.assertNotEqual(crp.check_policy(p, EXP, BLOB), [])

    def test_duplicate_key_fails(self):
        with self.assertRaises(ValueError):
            crp.load_strict('{"a": 1, "a": 2}')


class ContractChecks(unittest.TestCase):
    def test_unknown_check_in_contract_fails(self):
        c = copy.deepcopy(CONTRACT)
        c["milestones"]["8"]["acceptance"][0]["checks"] = ["ci / test: not real"]
        self.assertNotEqual(crp.check_contract(c, set(EXP)), [])

    def test_issue_missing_from_contract_fails(self):
        c = copy.deepcopy(CONTRACT)
        for entry in c["milestones"]["8"]["acceptance"]:
            if 164 in entry["issues"]:
                entry["issues"].remove(164)
        self.assertNotEqual(crp.check_contract(c, set(EXP)), [])


class DocChecks(unittest.TestCase):
    def test_doc_rules(self):
        self.assertNotEqual(crp.check_doc(""), [])
        self.assertNotEqual(crp.check_doc(DOC + " 4223a45"), [])
        self.assertNotEqual(crp.check_doc(DOC.replace("is not a pass", "x")), [])


if __name__ == "__main__":
    unittest.main()
