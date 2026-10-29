#!/usr/bin/env python3
"""Run issue 47's exact proposal filter and verify its full selected roster."""
import json
import os
import re
import subprocess
import sys


LISTED = re.compile(r'^(.+): test$')
RUNNING = re.compile(r'^running (\d+) tests?$')
ROW = re.compile(r'^test (.+) \.\.\. (.+)$')
SUMMARY = re.compile(r'^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; '
                     r'(\d+) ignored; (\d+) measured; (\d+) filtered out;')
COMMAND = ['cargo', 'test', '--locked', '-p', 'raft-runtime', '--test', 'it', 'proposal']


def selected_roster(output):
    names = [match.group(1) for line in output.splitlines() if (match := LISTED.fullmatch(line))]
    if not names or len(names) != len(set(names)) or any('proposal' not in name for name in names):
        raise ValueError('empty, duplicate or unexpected proposal test inventory')
    return set(names)


def verify_execution(output, expected):
    starts, summaries, rows = [], [], []
    for line in output.splitlines():
        if match := RUNNING.fullmatch(line):
            starts.append(int(match.group(1)))
        if match := SUMMARY.match(line):
            summaries.append(match.groups())
        if match := ROW.fullmatch(line):
            rows.append(match.groups())
    if len(starts) != 1 or len(summaries) != 1 or not expected:
        raise ValueError('missing or ambiguous proposal test summary')
    result, passed, failed, ignored, measured, filtered = summaries[0]
    names = [name for name, status in rows if status == 'ok']
    if (result != 'ok' or any(int(x) for x in (failed, ignored, measured))
            or starts[0] != len(expected) or int(passed) != len(expected)
            or len(names) != len(rows) or len(names) != len(set(names)) or set(names) != expected):
        raise ValueError('proposal test roster failed, skipped or incomplete')
    return {'selected': len(expected), 'filtered_other_tests': int(filtered), 'roster': sorted(expected)}


def main():
    env = dict(os.environ, CARGO_TERM_COLOR='never')
    listing = subprocess.run(COMMAND + ['--', '--list'], text=True, capture_output=True, env=env)
    sys.stdout.write(listing.stdout); sys.stderr.write(listing.stderr)
    if listing.returncode:
        raise ValueError('proposal test inventory command failed')
    expected = selected_roster(listing.stdout)
    run = subprocess.run(COMMAND, text=True, capture_output=True, env=env)
    sys.stdout.write(run.stdout); sys.stderr.write(run.stderr)
    if run.returncode:
        raise ValueError('proposal test command failed')
    print(json.dumps({'command': COMMAND, 'evidence': verify_execution(run.stdout, expected)}))


if __name__ == '__main__':
    main()
