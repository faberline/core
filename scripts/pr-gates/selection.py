"""Core test execution plan from the pinned AW selector. No lifecycle writes."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess


def api(path):
    return json.loads(subprocess.check_output(['gh','api',path],text=True))


def files(repo, number):
    rows=[]
    for page in range(1,31):
        batch=api(f'repos/{repo}/pulls/{number}/files?per_page=100&page={page}')
        if not isinstance(batch,list):raise ValueError('incomplete PR files')
        rows.extend(batch)
        if len(batch)<100:return rows
    raise ValueError('capped PR files cannot define coverage')


def planner(path):
    spec=importlib.util.spec_from_file_location('core_pinned_aw_test_plan',path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module


def pull_identity(pull):
    return dict(number=pull.get('number'),state=pull.get('state'),draft=pull.get('draft'),
                title=pull.get('title'),body=pull.get('body'),
                **{side:dict(sha=pull.get(side,{}).get('sha'),ref=pull.get(side,{}).get('ref'),
                            repo=pull.get(side,{}).get('repo',{}).get('full_name')) for side in ('head','base')})


def build(shared, policy, mode, repo, head, base, pull=None, changed=None, issue=None):
    pending=[]
    if mode=='diff':
        if (not isinstance(pull,dict) or pull.get('state')!='open' or pull.get('draft') is not False
                or pull.get('head',{}).get('sha')!=head or pull.get('base',{}).get('sha')!=base
                or any(pull.get(side,{}).get('repo',{}).get('full_name')!=repo for side in ('head','base'))):
            raise ValueError('PR moved or is foreign, closed or draft during planning')
        scope=shared.qualification().select(policy,pull,changed,issue)
        # A protected policy proposal must run every old gate before independent
        # review. This receipt only plans execution and cannot approve adoption.
        pending=[reason for reason in scope['issues'] if reason.startswith('gate infrastructure change requires reviewed policy update:')]
        other=[reason for reason in scope['issues'] if reason not in pending]
        if other:raise ValueError('; '.join(other))
        if pending:
            result=shared.select(policy,'all',repo,head,base)
        else:
            result=shared.select(policy,'diff',repo,head,base,pull=pull,files=changed,issue=issue)
    elif mode=='all':
        result=shared.select(policy,'all',repo,head,base)
    else:
        raise ValueError('unknown selection mode')
    # Current reviewed Core policy requires every gate for every changed path.
    # Keep the complete batches until a separate reviewed policy changes that.
    if set(result['selected'])!=set(policy['qualification']['checks']):
        raise ValueError('Core full fallback policy and complete batch roster differ')
    return dict(schema='CORE_TEST_SELECTION_V1',requested_mode=mode,test_plan=result,
                pending_protected_review=pending,qualification='not_evaluated')


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__,allow_abbrev=False)
    for key in ('root','baseline-root','output'):parser.add_argument('--'+key,type=Path,required=True)
    for key in ('expected-head','expected-base'):parser.add_argument('--'+key,required=True)
    args=parser.parse_args(argv)
    if any(not re.fullmatch('[0-9a-f]{40}',value) for value in (args.expected_head,args.expected_base)):
        raise ValueError('exact frozen commits required')
    # Reuse Core's existing clean checkout/ancestry enforcer.
    spec=importlib.util.spec_from_file_location('core_selection_identity',Path(__file__).with_name('plan.py'))
    unit=importlib.util.module_from_spec(spec);spec.loader.exec_module(unit)
    unit.verify_identity(args.root.resolve(),args.expected_head,args.expected_base,args.baseline_root.resolve())
    supplied=Path(os.environ['AW_ASSERTION_LIBRARY']).resolve()
    if supplied.name!='assertion_record.py':raise ValueError('pinned AW action library required')
    shared=planner(supplied.with_name('test_plan.py'))
    policy=json.loads((args.baseline_root/'.github/issue-ci-required-gates.json').read_text())
    repo=os.environ['GITHUB_REPOSITORY']
    if repo!='faberline/core':raise ValueError('foreign Core test-plan repository')
    event=os.environ['GITHUB_EVENT_NAME'];pull=changed=issue=None
    if event=='pull_request':
        payload=json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text())
        number=payload['number'];pull=api(f'repos/{repo}/pulls/{number}')
        changed=files(repo,number)
        linked=shared.qualification().linked_issue(pull)
        if linked is None:raise ValueError('canonical source issue required')
        issue=api(f'repos/{repo}/issues/{linked}');mode='diff'
    elif event=='push' and os.environ.get('GITHUB_REF','').startswith('refs/tags/v'):
        mode='all'
    else:
        raise ValueError('only a PR or verified release-tag push can plan tests')
    result=build(shared,policy,mode,repo,args.expected_head,args.expected_base,pull,changed,issue)
    if pull is not None:
        fresh=api(f'repos/{repo}/pulls/{pull["number"]}')
        if pull_identity(fresh)!=pull_identity(pull):
            raise ValueError('PR changed during complete selection')
    unit.verify_identity(args.root.resolve(),args.expected_head,args.expected_base,args.baseline_root.resolve())
    encoded=json.dumps(result,sort_keys=True)
    args.output.write_text(encoded+'\n')
    print('CORE_TEST_SELECTION_V1 '+encoded)


if __name__=='__main__':main()
