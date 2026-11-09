"""Read-only release contract checks; never tag, edit or call GitHub."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib

READY_RULE = 'all-milestone-issues-closed'
FULL_TEST_GATE = 'qualification'


def load_contract(path):
    contract = json.loads(Path(path).read_text())
    validate_contract(contract)
    return contract


def validate_contract(contract):
    if contract.get('version') != 1:
        raise ValueError('contract version must be 1')
    repo = contract.get('repo')
    if not isinstance(repo, str) or repo.count('/') != 1:
        raise ValueError('contract repo must be owner/name')
    if contract.get('ready_rule') != READY_RULE:
        raise ValueError(f'contract ready_rule must be {READY_RULE}')
    if contract.get('full_test_gate') != FULL_TEST_GATE:
        raise ValueError(f'contract full_test_gate must be {FULL_TEST_GATE}')
    milestones = contract.get('milestones')
    if not milestones:
        raise ValueError('contract empty milestones')
    for mid, entry in milestones.items():
        version = entry.get('version', '')
        if not re.fullmatch(r'\d+\.\d+\.\d+', str(version)):
            raise ValueError(f'milestone {mid} version must be X.Y.Z')
        if entry.get('tag') != 'v' + version:
            raise ValueError(f'milestone {mid} tag must be v{version}')
        if not str(entry.get('title', '')).endswith('@' + version):
            raise ValueError(f'milestone {mid} title must end with @{version}')
        goal = entry.get('goal', '')
        if hashlib.sha256(goal.encode('utf-8')).hexdigest() != entry.get('goal_sha256'):
            raise ValueError(f'milestone {mid} goal identity mismatch')
        issues = entry.get('issues')
        if (not isinstance(issues, list) or not issues
                or not all(isinstance(i, int) for i in issues)
                or len(set(issues)) != len(issues)):
            raise ValueError(f'milestone {mid} issues must be a non-empty list of unique ints')
        landed = entry.get('landed_prs')
        if not isinstance(landed, dict):
            raise ValueError(f'milestone {mid} landed_prs must be a dict')
        for key, prs in landed.items():
            if not (key.isdigit() and int(key) in issues):
                raise ValueError(f'milestone {mid} landed_prs key {key} is not a listed issue')
            if not (isinstance(prs, list) and all(isinstance(p, int) and p > 0 for p in prs)):
                raise ValueError(f'milestone {mid} landed_prs {key} must be a list of positive ints')


def workspace_version(cargo_text):
    return tomllib.loads(cargo_text)['workspace']['package']['version']


def version_holds(entry, cargo_text):
    actual = workspace_version(cargo_text)
    if actual == entry['version']:
        return []
    return [f"version-mismatch: contract {entry['version']}, Cargo.toml {actual}"]


def required_checks(gates):
    for rule in gates.get('qualification', {}).get('rules', []):
        if rule.get('paths') == ['**']:
            require = rule.get('require') or []
            if require:
                return list(require)
    raise ValueError('no full-test rule')


def evidence_holds(required, runs):
    by_name = {r['name']: r for r in runs}
    holds = []
    for name in required:
        run = by_name.get(name)
        if run is None:
            holds.append(f'missing-check: {name}')
            continue
        if run.get('conclusion') != 'success':
            holds.append(f'failed-check: {name}')
        tests = run.get('tests')
        if not (isinstance(tests, int) and not isinstance(tests, bool) and tests > 0):
            holds.append(f'zero-tests: {name}')
        if run.get('skipped') != 0:
            holds.append(f'skipped-tests: {name}')
    return holds


def promotion_holds(rc, formal):
    if rc['sha'] != formal['sha']:
        return [f"promotion-sha-mismatch: rc {rc['sha']}, formal {formal['sha']}"]
    return []


def readiness_holds(open_issues, closed_issues):
    holds = []
    if open_issues > 0:
        holds.append(f'open-milestone-issues: {open_issues}')
    if closed_issues == 0:
        holds.append('no-closed-issues')
    return holds


def landing_holds(entry, closed):
    return [f'missing-landed-pr: #{n}' for n in sorted(closed)
            if str(n) not in entry['landed_prs']]


def release_status(contract, milestone_id, open_issues, closed, cargo_text, gates, runs=None):
    if milestone_id not in contract['milestones']:
        raise ValueError('unknown milestone')
    entry = contract['milestones'][milestone_id]
    holds = (readiness_holds(open_issues, len(closed))
             + version_holds(entry, cargo_text)
             + landing_holds(entry, closed)
             + evidence_holds(required_checks(gates), runs or []))
    return {'milestone': milestone_id, 'version': entry['version'], 'tag': entry['tag'],
            'ready': holds == [], 'holds': holds}


def main(argv=None):
    parser = argparse.ArgumentParser(description='Read-only release contract checks.')
    sub = parser.add_subparsers(dest='command', required=True)
    check = sub.add_parser('check')
    check.add_argument('--root', default='.')
    check.add_argument('--milestone', required=True)
    check.add_argument('--open-issues', type=int, required=True)
    check.add_argument('--closed', default='')
    args = parser.parse_args(argv)
    root = Path(args.root)
    closed = [int(x) for x in args.closed.split(',') if x.strip()]
    try:
        contract = load_contract(root / '.github' / 'aw-release.json')
        cargo_text = (root / 'Cargo.toml').read_text()
        gates = json.loads((root / '.github' / 'issue-ci-required-gates.json').read_text())
        result = release_status(contract, args.milestone, args.open_issues, closed,
                                cargo_text, gates, runs=None)
    except ValueError as err:
        print(f'refused: {err}', file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0 if result['ready'] else 1


if __name__ == '__main__':
    sys.exit(main())
