#!/usr/bin/env python3
"""Check the release test policy and contract (issue #226, part 3 of #223).

Checks .github/aw-release-gates.json against the jobs that release.yml
produces, .github/aw-release.json against the reviewed milestone contract,
and docs/operations/release-pipeline.md against the required procedure text.
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent

RELEASE_YML = ".github/workflows/release.yml"
CI_YML = ".github/workflows/ci.yml"
GATES_JSON = ".github/aw-release-gates.json"
CONTRACT_JSON = ".github/aw-release.json"
DOC = "docs/operations/release-pipeline.md"

GOAL_SHA256 = "f6dbdbcd0580925139de1bf18f1626c057fbb073663d638d7ab25aa9666c200e"
MILESTONE_8_ISSUES = {
    23, 40, 42, 45, 47, 48, 49, 73, 90, 92, 97, 99,
    156, 158, 159, 160, 161, 162, 163, 164,
}

MATRIX_RE = re.compile(r"\$\{\{\s*matrix\.([A-Za-z0-9_]+)\s*\}\}")
HEX40_RE = re.compile(r"^[0-9a-f]{40}$")
DOC_REQUIRED = [
    "v0.5.0-rc.1",
    "v0.5.0",
    "aw release-status --milestone 8",
    "aw release-advance --milestone 8 --expected <fingerprint>",
    "aw release-preflight --tag <tag> --commit <sha>",
    "release.yml",
    "aw-release-gates.json",
    "aw-release.json",
    "is not a pass",
]
DOC_FORBIDDEN = ["4223a45", "come back"]


def _reject_duplicates(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError(f"duplicate key: {key}")
        out[key] = value
    return out


def load_strict(text: str) -> dict:
    return json.loads(text, object_pairs_hook=_reject_duplicates)


def job_names(job_id: str, job: dict) -> list:
    name = job.get("name", job_id)
    strategy = job.get("strategy") or {}
    matrix = strategy.get("matrix") or {}
    if "include" not in matrix:
        return [name]
    names = []
    for row in matrix["include"]:
        def repl(m, row=row):
            return str(row[m.group(1)])
        names.append(MATRIX_RE.sub(repl, name))
    return names


def expand_names(release_yaml: str, ci_yaml: str) -> list:
    release = yaml.safe_load(release_yaml)
    ci = yaml.safe_load(ci_yaml)
    names = []
    for job_id, job in release["jobs"].items():
        uses = job.get("uses")
        if uses is None:
            names.extend(job_names(job_id, job))
            continue
        if not str(uses).startswith("./"):
            raise ValueError(f"unsupported uses in release.yml: {uses}")
        if uses != "./.github/workflows/ci.yml":
            raise ValueError(f"unsupported called workflow: {uses}")
        caller = job.get("name", job_id)
        for ci_job_id, ci_job in ci["jobs"].items():
            for n in job_names(ci_job_id, ci_job):
                names.append(f"{caller} / {n}")
    return names


def kind_for(name: str) -> str:
    if name.startswith("ci / "):
        name = name[len("ci / "):]
    if name == "preflight" or name == "gate" or name.startswith("lint: "):
        return "policy"
    if name.startswith("test: "):
        return "unit"
    if name.startswith("e2e: "):
        return "functional_e2e"
    if name.startswith("perf: "):
        return "performance"
    raise ValueError(f"no kind for job name: {name}")


def expected_jobs(release_yaml: str, ci_yaml: str) -> dict:
    names = expand_names(release_yaml, ci_yaml)
    out = {}
    for name in names:
        if name in out:
            raise ValueError(f"duplicate job name: {name}")
        out[name] = kind_for(name)
    return out


def check_policy(policy: dict, expected: dict, blob_sha: str) -> list:
    errors = []
    if set(policy) != {"version", "repo", "workflows"}:
        errors.append("top-level keys must be version, repo, workflows")
    if policy.get("version") != 1:
        errors.append("version must be 1")
    if policy.get("repo") != "faberline/core":
        errors.append("repo must be faberline/core")
    workflows = policy.get("workflows")
    if not isinstance(workflows, list) or len(workflows) != 1 or not isinstance(workflows[0], dict):
        errors.append("workflows must be a list of exactly one object")
        return errors
    wf = workflows[0]
    if set(wf) != {"path", "id", "definition_sha", "jobs"}:
        errors.append("workflow keys must be path, id, definition_sha, jobs")
    if wf.get("path") != RELEASE_YML:
        errors.append(f"path must be {RELEASE_YML}")
    wid = wf.get("id")
    if not isinstance(wid, int) or isinstance(wid, bool) or wid <= 0:
        errors.append("id must be an integer greater than 0")
    sha = wf.get("definition_sha")
    if sha != blob_sha:
        errors.append(f"definition_sha {sha} does not equal blob hash {blob_sha}")
    if not isinstance(sha, str) or not HEX40_RE.match(sha):
        errors.append("definition_sha must be 40 lowercase hex characters")
    jobs = wf.get("jobs")
    if not isinstance(jobs, dict):
        errors.append("jobs must be an object")
        return errors
    for name in sorted(expected):
        if name not in jobs:
            errors.append(f"missing job: {name}")
        elif jobs[name] != expected[name]:
            errors.append(
                f"wrong kind for {name}: {jobs[name]} (expected {expected[name]})"
            )
    for name in sorted(jobs):
        if name not in expected:
            errors.append(f"extra job: {name}")
    values = set(jobs.values())
    for kind in ("unit", "functional_e2e", "performance"):
        if kind not in values:
            errors.append(f"kind {kind} does not appear")
    return errors


def check_contract(contract: dict, job_names: set) -> list:
    errors = []
    if set(contract) != {"version", "repo", "milestones"}:
        errors.append("top-level keys must be version, repo, milestones")
    if contract.get("version") != 1:
        errors.append("version must be 1")
    if contract.get("repo") != "faberline/core":
        errors.append("repo must be faberline/core")
    milestones = contract.get("milestones")
    if not isinstance(milestones, dict) or set(milestones) != {"8"}:
        errors.append("milestones must have exactly the key 8")
        return errors
    m = milestones["8"]
    if not isinstance(m, dict) or set(m) != {"version", "goal_sha256", "acceptance", "publication"}:
        errors.append("milestone 8 keys must be version, goal_sha256, acceptance, publication")
        return errors
    if m["version"] != "0.5.0":
        errors.append("milestone 8 version must be 0.5.0")
    if m["goal_sha256"] != GOAL_SHA256:
        errors.append("milestone 8 goal_sha256 does not match")
    if m["publication"] != "github-release":
        errors.append("publication must be github-release")
    acceptance = m["acceptance"]
    if not isinstance(acceptance, list) or not acceptance:
        errors.append("acceptance must be a non-empty list")
        return errors
    seen = []
    for i, entry in enumerate(acceptance):
        if not isinstance(entry, dict) or set(entry) != {"goal", "issues", "checks"}:
            errors.append(f"acceptance[{i}] keys must be goal, issues, checks")
            continue
        if not isinstance(entry["goal"], str) or not entry["goal"]:
            errors.append(f"acceptance[{i}] goal must be a non-empty string")
        issues = entry["issues"]
        if not isinstance(issues, list) or not issues or not all(
            isinstance(x, int) and not isinstance(x, bool) for x in issues
        ):
            errors.append(f"acceptance[{i}] issues must be a non-empty list of integers")
        else:
            seen.extend(issues)
        checks = entry["checks"]
        if not isinstance(checks, list) or not checks or not all(
            isinstance(c, str) for c in checks
        ):
            errors.append(f"acceptance[{i}] checks must be a non-empty list of strings")
        else:
            for c in checks:
                if c not in job_names:
                    errors.append(f"acceptance[{i}] unknown check: {c}")
    if len(seen) != len(set(seen)):
        errors.append("an issue appears in more than one acceptance entry")
    if set(seen) != MILESTONE_8_ISSUES:
        missing = sorted(MILESTONE_8_ISSUES - set(seen))
        extra = sorted(set(seen) - MILESTONE_8_ISSUES)
        if missing:
            errors.append(f"acceptance misses issues: {missing}")
        if extra:
            errors.append(f"acceptance has unknown issues: {extra}")
    return errors


def check_doc(text: str) -> list:
    errors = []
    for s in DOC_REQUIRED:
        if s not in text:
            errors.append(f"doc misses: {s}")
    for s in DOC_FORBIDDEN:
        if s in text:
            errors.append(f"doc contains forbidden text: {s}")
    return errors


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--print-jobs",
        action="store_true",
        help="print the job map derived from release.yml and ci.yml",
    )
    args = parser.parse_args(argv)

    release_text = (ROOT / RELEASE_YML).read_text(encoding="utf-8")
    ci_text = (ROOT / CI_YML).read_text(encoding="utf-8")

    if args.print_jobs:
        print(json.dumps(expected_jobs(release_text, ci_text), indent=2))
        return 0

    policy = load_strict((ROOT / GATES_JSON).read_text(encoding="utf-8"))
    contract = load_strict((ROOT / CONTRACT_JSON).read_text(encoding="utf-8"))
    doc = (ROOT / DOC).read_text(encoding="utf-8")
    blob_sha = subprocess.run(
        ["git", "hash-object", RELEASE_YML],
        capture_output=True,
        text=True,
        check=True,
        cwd=ROOT,
    ).stdout.strip()
    expected = expected_jobs(release_text, ci_text)

    errors = []
    errors += check_policy(policy, expected, blob_sha)
    errors += check_contract(contract, set(expected))
    errors += check_doc(doc)
    if errors:
        for e in errors:
            print(f"FAIL: {e}", file=sys.stderr)
        return 1
    print("release policy ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
