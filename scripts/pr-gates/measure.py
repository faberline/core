#!/usr/bin/env python3
"""Retain real HTTP/2 and allocator measurements at their existing bounds."""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys

SPEC = importlib.util.spec_from_file_location('core_contracts', Path(__file__).with_name('contracts.py'))
c = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(c)


def measurements(stdout, stderr, kind):
    evidence = c.unit.libtest_evidence(stdout, 1)
    roster = evidence[0]['roster']
    text = stdout + '\n' + stderr
    if kind == 'http2':
        if not any(n.endswith('::generated_python_h2c_runtime_reused_connection_perf_guard') for n in roster):
            raise ValueError('required HTTP/2 measurement test did not execute')
        rows=re.findall(r'^h2c_generated_sync_reused_connection seconds=([0-9.]+) requests=(\d+)$',text,re.M)
        if len(rows)!=1:
            raise ValueError('missing or duplicate HTTP/2 measurement')
        seconds=float(rows[0][0]);requests=int(rows[0][1])
        if not math.isfinite(seconds) or not 0 < seconds < 5.0 or requests!=128:
            raise ValueError('HTTP/2 original 128-request / 5-second threshold failed')
        return dict(tests=evidence,measurement=dict(seconds=seconds,requests=requests,seconds_exclusive_limit=5.0))
    if kind!='snapshot':
        raise ValueError('unknown measured workload')
    if 'snapshot_peak_memory_grows_sublinearly' not in roster:
        raise ValueError('required allocator measurement test did not execute')
    rows=re.findall(r'^CORE_SNAPSHOT_PEAK small_entries=(\d+) small_peak_bytes=(\d+) large_entries=(\d+) large_peak_bytes=(\d+) limit_bytes=(\d+)$',text,re.M)
    if len(rows)!=1:
        raise ValueError('missing or duplicate allocator measurement')
    small_count,small_peak,large_count,large_peak,limit=map(int,rows[0])
    if (small_count,large_count)!=(1000,50000) or limit!=max(small_peak,64*1024)*4 or not 0<=large_peak<limit:
        raise ValueError('snapshot original sublinear allocator threshold failed')
    return dict(tests=evidence,measurement=dict(small_entries=small_count,small_peak_bytes=small_peak,
        large_entries=large_count,large_peak_bytes=large_peak,bytes_exclusive_limit=limit))


def main(argv=None):
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root',type=Path,required=True);p.add_argument('--baseline-root',type=Path,required=True)
    p.add_argument('--expected-head',required=True);p.add_argument('--expected-base',required=True)
    a=p.parse_args(argv)
    identity=c.unit.verify_identity(a.root,a.expected_head,a.expected_base,a.baseline_root)
    packages=c.load(a.root);c.compare_inventory(c.load(a.baseline_root),packages)
    workloads=[('http2','openapi-codegen',['--lib']),('snapshot','raft-runtime',['--test','snapshot_peak_memory'])]
    errors=[]
    for kind,package,selector in workloads:
        c.unit.verify_identity(a.root,a.expected_head,a.expected_base,a.baseline_root)
        command=['cargo','test','--release','--locked','-p',package,*selector,'--all-features','--',
                 '--include-ignored','--format','pretty','--color','never','--test-threads','1','--nocapture']
        result=subprocess.run(command,cwd=a.root,capture_output=True,text=True)
        sys.stdout.write(result.stdout);sys.stdout.flush();sys.stderr.write(result.stderr);sys.stderr.flush()
        record=dict(schema='CORE_PERFORMANCE_EVIDENCE_V1',workload=kind,command=command,provenance=identity,
            run_id=os.environ.get('GITHUB_RUN_ID'),run_attempt=os.environ.get('GITHUB_RUN_ATTEMPT'),
            stdout_sha256=hashlib.sha256(result.stdout.encode()).hexdigest(),stderr_sha256=hashlib.sha256(result.stderr.encode()).hexdigest())
        try:
            if result.returncode:raise ValueError('measured target failed with exit '+str(result.returncode))
            record.update(measurements(result.stdout,result.stderr,kind),result='pass')
        except ValueError as error:
            record.update(result='fail',error=str(error));errors.append(kind+': '+str(error))
        print('CORE_PERFORMANCE_EVIDENCE_V1 '+json.dumps(record,sort_keys=True),flush=True)
    if errors:raise ValueError('\n'.join(errors))


if __name__=='__main__':main()
