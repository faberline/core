#!/usr/bin/env python3
"""Core-owned batches. Preserve every manifest row and every prior gate."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys


def sibling(name):
    spec = importlib.util.spec_from_file_location('core_batch_' + name,
                                                Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


unit = sibling('plan')
contracts = sibling('contracts')
LEGACY = [('service-k8s', []), ('storage-durable', []),
          ('service-backup', ['--features', 'http-client']), ('raft-core', []), ('raft-runtime', [])]
RAFT = {
    'proposal': ['python3', 'scripts/pr-gates/proposal_gate.py'],
    'raft-full': ['cargo', 'test', '--locked', '-p', 'raft-core', '-p', 'raft-runtime', '--no-fail-fast'],
    'clippy': ['cargo', 'clippy', '--locked', '-p', 'raft-runtime', '--all-targets', '--', '-D', 'warnings'],
}
OTHER = {'candidate workspace unit inventory', 'candidate workspace format',
         'candidate source issue contract inspection', 'development preflight',
         'candidate measured performance'}
STATIC = {'candidate workspace format', 'candidate source issue contract inspection'}


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def count_config(value):
    if (not isinstance(value, dict) or set(value) != {'version', 'unit_batches', 'functional_batches'}
            or type(value['version']) is not int or value['version'] != 1
            or any(type(value[k]) is not int or not 1 <= value[k] <= 32
                   for k in ('unit_batches', 'functional_batches'))):
        raise ValueError('invalid Core batch configuration')
    return value


def cases(unit_rows, contract_rows, policy):
    """Match fresh complete matrices to stable reviewed check IDs, without filters."""
    expected = []
    for runner, rows, kind, prefix in [('unit', unit_rows, 'unit', 'candidate unit-static'),
                                     ('contracts', contract_rows, 'functional_e2e', 'candidate contracts')]:
        for row in rows:
            expected.append(dict(runner=runner, kind=kind, package=row['package'],
                                 profile=row['profile'],
                                 label=f"{prefix} ({row['package']}, {row['profile']})"))
    for package, args in LEGACY:
        expected.append(dict(runner='legacy', kind='policy', package=package, label=package,
                             command=['cargo', 'test', '--locked', '-p', package, *args]))
    for gate, command in RAFT.items():
        expected.append(dict(runner='raft', kind='policy', package='raft-runtime',
                             label=f'candidate issue 47 ({gate})', command=command))
    registry = policy['qualification']['checks']
    labels = [c.get('case_label', c['job']) for c in registry.values()]
    wanted = [c['label'] for c in expected]
    if (len(labels) != len(set(labels)) or len(wanted) != len(set(wanted))
            or set(labels) != set(wanted) | OTHER):
        raise ValueError('missing, duplicate or unregistered complete case roster')
    by_label = {c.get('case_label', c['job']):(name, c) for name, c in registry.items()}
    for case in expected:
        name, check = by_label[case['label']]
        if check['kind'] != case['kind'] or check.get('case_id') != name:
            raise ValueError('case category or identity changed: ' + name)
        case['id'] = name
    return expected


def shards(catalog, count):
    if type(count) is not int or not 1 <= count <= len(catalog):
        raise ValueError('empty or excessive batch count')
    groups = {}
    for case in catalog:
        groups.setdefault(case['package'], []).append(case)
    groups = list(groups.values())
    while len(groups) < count:
        group = max(groups, key=lambda g:(len(g), g[0]['package']))
        groups.remove(group)
        middle = len(group) // 2
        groups.extend([group[:middle], group[middle:]])
    result = [[] for _ in range(count)]
    for group in sorted(groups, key=lambda g:(-len(g), g[0]['package'], g[0]['id'])):
        index = min(range(count), key=lambda i:(len(result[i]), i))
        result[index].extend(group)
    if any(not row for row in result):
        raise ValueError('zero-case batch')
    return [sorted(row, key=lambda c:c['id']) for row in result]


def build(unit_rows, contract_rows, policy, settings):
    settings = count_config(settings)
    catalog = cases(unit_rows, contract_rows, policy)
    result = dict(version=1, groups={})
    for group in ('unit', 'functional'):
        selected = [c for c in catalog if (c['runner'] == 'unit') == (group == 'unit')]
        batches = shards(selected, settings[group + '_batches'])
        result['groups'][group] = []
        for index, rows in enumerate(batches, 1):
            batch = f'{index:02}/{len(batches):02}'
            job = f'candidate {group} batch ({batch})'
            if any(policy['qualification']['checks'][c['id']]['job'] != job for c in rows):
                raise ValueError('batch layout differs from reviewed policy: ' + job)
            result['groups'][group].append(dict(batch=batch, job=job, cases=rows))
    expected = set(policy['development_pipeline']['grouped_checks'])
    static = {name for name, c in policy['qualification']['checks'].items()
              if c.get('case_label') in STATIC}
    if {c['id'] for c in catalog} | static != expected:
        raise ValueError('grouped case registry differs from complete roster')
    result['sha256'] = digest(result)
    return result


def matrix(plan, group):
    return {'include': [dict(batch=b['batch'], openapi=any(c['package']=='openapi-codegen' for c in b['cases']),
                             kind=any(c['runner']=='contracts' and c['package']=='service-k8s' for c in b['cases']))
                        for b in plan['groups'][group]]}


def commands(case, root, baseline, head, base):
    common = ['--root', str(root), '--baseline-root', str(baseline), '--expected-head', head, '--expected-base', base]
    driver = root / 'scripts/pr-gates'
    if case['runner'] == 'unit':
        row = ['--package', case['package'], '--profile', case['profile']]
        return [[sys.executable, str(driver/'plan.py'), 'run', *common, *row, '--kind', kind]
                for kind in ('unit', 'clippy')] + [[sys.executable, str(driver/'docs.py'), *common, *row]]
    if case['runner'] == 'contracts':
        return [[sys.executable, str(driver/'contracts.py'), 'run', *common,
                 '--package', case['package'], '--profile', case['profile']]]
    return [case['command']]


def execute(plan, group, batch, root, baseline, head, base, invoke=subprocess.run):
    selected = [b for b in plan['groups'][group] if b['batch'] == batch]
    if len(selected) != 1 or not selected[0]['cases']:
        raise ValueError('unknown or zero-case batch')
    roster = selected[0]['cases']
    ids = [c['id'] for c in roster]
    if len(ids) != len(set(ids)):
        raise ValueError('duplicate batch case')
    results = []
    for case in roster:
        print('::group::' + case['id'] + ' ' + case['label'], flush=True)
        errors = []
        env = dict(os.environ)
        # A legacy command must not inherit the contract-only live-cluster flag.
        env.pop('SERVICE_K8S_LIVE_KUBE', None)
        live = case['runner']=='contracts' and case['package']=='service-k8s'
        kind = str(Path(os.environ.get('RUNNER_TEMP', '/tmp'))/'kind')
        try:
            unit.verify_identity(root, head, base, baseline)
            if case['runner']=='contracts':
                env['SERVICE_K8S_LIVE_KUBE'] = '1'
                (root/'.aw').mkdir(exist_ok=True)
            if live:
                setup = [[kind, 'create', 'cluster', '--name', 'core-contracts', '--wait', '120s'],
                         ['kubectl', 'config', 'current-context'], ['kubectl', 'get', 'nodes']]
                for argv in setup:
                    if invoke(argv, cwd=root, env=env).returncode:
                        raise ValueError('live Kind setup failed')
                current = subprocess.check_output(['kubectl', 'config', 'current-context'], cwd=root, env=env, text=True).strip()
                if current != 'kind-core-contracts':raise ValueError('wrong Kind context')
            for argv in commands(case, root, baseline, head, base):
                print(json.dumps(dict(case=case['id'], command=argv)), flush=True)
                code = invoke(argv, cwd=root, env=env).returncode
                if code:errors.append(dict(command=argv, exit_code=code))
        except (ValueError, OSError) as error:
            errors.append(dict(error=str(error)))
        finally:
            if live:
                try:
                    if invoke([kind, 'delete', 'cluster', '--name', 'core-contracts'], cwd=root, env=env).returncode:
                        errors.append(dict(error='Kind cleanup failed'))
                except OSError as error:errors.append(dict(error='Kind cleanup: ' + str(error)))
            try:unit.verify_identity(root, head, base, baseline)
            except (ValueError, OSError) as error:errors.append(dict(error=str(error)))
        record = dict(case_id=case['id'], case_label=case['label'], errors=errors, result='fail' if errors else 'pass')
        results.append(record)
        print('CORE_BATCH_CASE_V1 ' + json.dumps(record, sort_keys=True), flush=True)
        print('::endgroup::', flush=True)
    if [r['case_id'] for r in results] != ids:
        raise ValueError('batch execution differs from complete roster')
    summary = dict(schema='CORE_BATCH_EVIDENCE_V1', head=head, base=base, plan_sha256=plan['sha256'],
                   group=group, batch=batch, expected=ids, results=results,
                   run_id=os.environ.get('GITHUB_RUN_ID'), attempt=os.environ.get('GITHUB_RUN_ATTEMPT'))
    print('CORE_BATCH_EVIDENCE_V1 ' + json.dumps(summary, sort_keys=True), flush=True)
    if any(r['result'] != 'pass' for r in results):
        raise ValueError('one or more batch cases failed')
    return summary


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('action', choices=['matrix', 'run'])
    for name in ('root', 'baseline-root'):p.add_argument('--'+name, type=Path, required=True)
    for name in ('expected-head', 'expected-base', 'group'):p.add_argument('--'+name, required=True)
    p.add_argument('--batch')
    args = p.parse_args(argv)
    if args.group not in ('unit', 'functional'):p.error('unknown batch group')
    root, baseline = args.root.resolve(), args.baseline_root.resolve()
    unit.verify_identity(root, args.expected_head, args.expected_base, baseline)
    packages, prior = unit.load(root), unit.load(baseline)
    unit.compare_inventory(prior, packages)
    integration, old_integration = contracts.load(root), contracts.load(baseline)
    contracts.compare_inventory(old_integration, integration)
    policy = json.loads((root/'.github/issue-ci-required-gates.json').read_text())
    settings = json.loads((root/'.github/development-test-batches.json').read_text())
    plan = build(unit.matrix(packages)['include'], contracts.matrix(integration)['include'], policy, settings)
    unit.verify_identity(root, args.expected_head, args.expected_base, baseline)
    if args.action == 'matrix':
        print(json.dumps(dict(sha256=plan['sha256'], cases=sum(len(b['cases']) for b in plan['groups'][args.group]))), file=sys.stderr)
        print(json.dumps(matrix(plan, args.group), separators=(',', ':')))
    else:
        execute(plan, args.group, args.batch, root, baseline, args.expected_head, args.expected_base)


if __name__ == '__main__':main()
