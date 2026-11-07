#!/usr/bin/env python3
"""Build library documentation and run every collected doctest per profile.

An empty doctest collection is recorded as such, never as unit-test evidence.
The separate mandatory unit gate still refuses every empty unit target.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

SPEC = importlib.util.spec_from_file_location('core_doc_unit', Path(__file__).with_name('plan.py'))
unit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(unit)


def evidence(stdout, allow_ignored=False):
    blocks=[];current=None
    for line in stdout.splitlines():
        running=unit.RUNNING.fullmatch(line);summary=unit.SUMMARY.match(line);row=unit.TEST_ROW.fullmatch(line)
        if running:
            if current is not None:raise ValueError('partial doctest harness')
            current=dict(running=int(running[1]),roster=[],ignored_roster=[])
        elif summary:
            if current is None:raise ValueError('doctest summary without harness')
            counts=dict(zip(('passed','failed','ignored','measured','filtered'),map(int,summary.groups()[1:])))
            if summary[1]!='ok' or any(counts[k]for k in ['failed','measured','filtered']) or(counts['ignored']and not allow_ignored):
                raise ValueError('nonpassing doctest counts')
            if current['running']!=counts['passed']+counts['ignored']or len(current['roster'])!=counts['passed']or len(current['ignored_roster'])!=counts['ignored']:
                raise ValueError('partial doctest roster')
            blocks.append(dict(current,**counts));current=None
        elif row:
            if current is None or row[1]in current['roster']+current['ignored_roster']:raise ValueError('invalid doctest row')
            if row[2]=='ok':current['roster'].append(row[1])
            elif allow_ignored and(row[2]=='ignored'or row[2].startswith('ignored, ')):current['ignored_roster'].append(row[1])
            else:raise ValueError('nonpassing doctest row')
    if current is not None or not blocks:raise ValueError('missing doctest harness')
    return dict(harnesses=blocks,counts_as_unit_evidence=False,
                collected_examples=sum(b['running']for b in blocks))


def metadata(root):
    data=json.loads(subprocess.check_output(['cargo','metadata','--locked','--no-deps','--format-version','1'],cwd=root,text=True))
    return data,unit.inventory(root,data)


def execute(argv,root):
    result=subprocess.run(argv,cwd=root,capture_output=True,text=True)
    sys.stdout.write(result.stdout);sys.stdout.flush();sys.stderr.write(result.stderr);sys.stderr.flush()
    record=dict(command=argv,exit_code=result.returncode,stdout_sha256=hashlib.sha256(result.stdout.encode()).hexdigest(),stderr_sha256=hashlib.sha256(result.stderr.encode()).hexdigest())
    if result.returncode:raise ValueError('documentation command failed: '+json.dumps(record,sort_keys=True))
    return result,record


def run(root,package,profile,provenance):
    args=['--locked','-p',package,*unit.feature_args(profile)]
    _,build=execute(['cargo','doc',*args,'--no-deps'],root)
    command=['cargo','test',*args,'--doc','--','--format','pretty','--color','never']
    result,test=execute(command,root);debug=evidence(result.stdout,allow_ignored=True)
    ignored={n for b in debug['harnesses']for n in b['ignored_roster']}
    record=dict(schema='CORE_DOC_EVIDENCE_V1',provenance=provenance,package=package,profile=profile,
                build=build,default_test=test,default_evidence=debug)
    if ignored:
        command=['cargo','test','--release',*args,'--doc','--','--include-ignored','--format','pretty','--color','never']
        result,release=execute(command,root);full=evidence(result.stdout)
        executed={n for b in full['harnesses']for n in b['roster']}
        if not ignored<=executed:raise ValueError('ignored documentation examples did not all execute')
        record.update(release_test=release,release_evidence=full,resolved_ignored=sorted(ignored))
    else:record['default_evidence']=evidence(result.stdout)
    record['result']='pass'
    print('CORE_DOC_EVIDENCE_V1 '+json.dumps(record,sort_keys=True),flush=True)


def main(argv=None):
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['root','baseline-root']:p.add_argument('--'+name,type=Path,required=True)
    for name in ['expected-head','expected-base','package','profile']:p.add_argument('--'+name,required=True)
    a=p.parse_args(argv)
    identity=unit.verify_identity(a.root,a.expected_head,a.expected_base,a.baseline_root)
    raw,packages=metadata(a.root);_,baseline=metadata(a.baseline_root);unit.compare_inventory(baseline,packages)
    package=next((x for x in packages if x['name']==a.package),None)
    if package is None or a.profile not in unit.profiles(package):p.error('unknown manifest-derived row')
    original=next(x for x in raw['packages']if x['id']in raw['workspace_members']and x['name']==a.package)
    libraries=[t for t in original['targets']if set(t['kind'])&unit.LIB_KINDS]
    if not libraries or any(t.get('doctest')is not True for t in libraries):
        raise ValueError('missing or disabled documentation target requires reviewed applicability')
    identity=unit.verify_identity(a.root,a.expected_head,a.expected_base,a.baseline_root)
    identity.update(run_id=os.environ.get('GITHUB_RUN_ID'),run_attempt=os.environ.get('GITHUB_RUN_ATTEMPT'),
                    driver_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    try:
        run(a.root,a.package,a.profile,identity)
    finally:
        unit.verify_identity(a.root,a.expected_head,a.expected_base,a.baseline_root)


if __name__=='__main__':main()
