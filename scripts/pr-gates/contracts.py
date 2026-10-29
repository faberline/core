#!/usr/bin/env python3
"""Complete integration-target execution with current-head evidence.

This read-only runner does not approve a PR, close an issue or change policy.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import tomllib

SPEC = importlib.util.spec_from_file_location('core_unit_plan', Path(__file__).with_name('plan.py'))
unit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(unit)


def inventory(root, metadata):
    packages = unit.inventory(root, metadata)
    members = {p['name']: p for p in metadata['packages'] if p['id'] in metadata['workspace_members']}
    for package in packages:
        source = members[package['name']]
        manifest = tomllib.loads(Path(source['manifest_path']).read_text())
        declarations = {t['name']: t for t in manifest.get('test', [])}
        targets = []
        for target in source['targets']:
            if target['kind'] != ['test']:
                continue
            declaration = declarations.get(target['name'], {})
            if not target.get('test', True) or declaration.get('harness', True) is not True:
                raise ValueError('disabled integration harness requires reviewed policy: ' + package['name'])
            targets.append({'name': target['name'], 'kind': ['test'],
                            'required-features': target.get('required-features', []),
                            'harness': True, 'test': True})
        package['integration_targets'] = sorted(targets, key=lambda t: t['name'])
    return packages


def load(root):
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'], cwd=root, text=True))
    return inventory(root, metadata)


def active_targets(package, profile):
    if profile not in unit.profiles(package):
        raise ValueError('unknown manifest-derived profile')
    enabled = unit.enabled_features(package, profile)
    return [t for t in package['integration_targets'] if set(t['required-features']) <= enabled]


def matrix(packages):
    rows = []
    for package in packages:
        reached = set()
        for profile in unit.profiles(package):
            active = active_targets(package, profile)
            reached.update(t['name'] for t in active)
            if active:
                rows.append({'package': package['name'], 'profile': profile})
        if reached != {t['name'] for t in package['integration_targets']}:
            raise ValueError('unreachable integration target: ' + package['name'])
    if not rows or len(rows) > 256:
        raise ValueError('empty or oversized integration matrix')
    return {'include': sorted(rows, key=lambda r: (r['profile'] != 'default', r['package'], r['profile']))}


def compare_inventory(baseline, head):
    unit.compare_inventory(baseline, head)
    current = {p['name']: p for p in head}
    for before in baseline:
        after = current[before['name']]
        wanted = {t['name'] for t in before['integration_targets']}
        actual = {t['name'] for t in after['integration_targets']}
        if not wanted <= actual:
            raise ValueError('integration target removal requires reviewed policy: ' + before['name'])
        for profile in unit.profiles(before):
            old = {t['name'] for t in active_targets(before, profile)}
            new = {t['name'] for t in active_targets(after, profile)}
            if not old <= new:
                raise ValueError('formerly enabled integration target lost: ' + before['name'] + '/' + profile)


def command(package, profile, target):
    if target not in active_targets(package, profile):
        raise ValueError('target outside complete manifest-derived row')
    return ['cargo', 'test', '--release', '--locked', '-p', package['name'],
            '--test', target['name'], *unit.feature_args(profile), '--',
            '--include-ignored', '--format', 'pretty', '--color', 'never', '--test-threads', '1']


def run(root, package, profile, provenance):
    targets = active_targets(package, profile)
    if not targets:
        raise ValueError('empty integration row')
    errors = []
    for target in targets:
        argv = command(package, profile, target)
        started = time.monotonic()
        result = subprocess.run(argv, cwd=root, capture_output=True, text=True)
        elapsed = time.monotonic() - started
        sys.stdout.write(result.stdout)
        sys.stdout.flush()
        sys.stderr.write(result.stderr)
        sys.stderr.flush()
        record = {'schema': 'CORE_CONTRACT_EVIDENCE_V1', 'provenance': provenance,
                  'package': package['name'], 'profile': profile, 'target': target['name'],
                  'command': argv, 'elapsed_seconds': elapsed, 'exit_code': result.returncode,
                  'stdout_sha256': hashlib.sha256(result.stdout.encode()).hexdigest(),
                  'stderr_sha256': hashlib.sha256(result.stderr.encode()).hexdigest()}
        try:
            if result.returncode:
                raise ValueError('product command failed with exit ' + str(result.returncode))
            record['tests'] = unit.libtest_evidence(result.stdout, 1)
            record['result'] = 'pass'
        except ValueError as error:
            errors.append(target['name'] + ': ' + str(error))
            record.update(result='fail', error=str(error))
        print('CORE_CONTRACT_EVIDENCE_V1 ' + json.dumps(record, sort_keys=True), flush=True)
    if errors:
        raise ValueError('\n'.join(errors))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['matrix', 'run'])
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--baseline-root', type=Path, required=True)
    parser.add_argument('--expected-head', required=True)
    parser.add_argument('--expected-base', required=True)
    parser.add_argument('--package')
    parser.add_argument('--profile')
    args = parser.parse_args(argv)
    identity = unit.verify_identity(args.root, args.expected_head, args.expected_base, args.baseline_root)
    packages = load(args.root)
    baseline = load(args.baseline_root)
    compare_inventory(baseline, packages)
    expected = matrix(packages)
    if args.action == 'matrix':
        unit.verify_identity(args.root, args.expected_head, args.expected_base, args.baseline_root)
        print(json.dumps(expected, separators=(',', ':')))
        return
    if {'package': args.package, 'profile': args.profile} not in expected['include']:
        parser.error('run requires a complete manifest-derived integration row')
    package = next(p for p in packages if p['name'] == args.package)
    identity = unit.verify_identity(args.root, args.expected_head, args.expected_base, args.baseline_root)
    identity.update(repository='faberline/core', run_id=os.environ.get('GITHUB_RUN_ID'),
                    run_attempt=os.environ.get('GITHUB_RUN_ATTEMPT'),
                    driver_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                    unit_driver_sha256=hashlib.sha256(Path(unit.__file__).read_bytes()).hexdigest())
    try:
        run(args.root, package, args.profile, identity)
    finally:
        unit.verify_identity(args.root, args.expected_head, args.expected_base, args.baseline_root)


if __name__ == '__main__':
    main()
