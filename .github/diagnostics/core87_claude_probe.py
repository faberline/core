"""Read-only model transport probe. This cannot approve or merge a PR."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import re

ROOT = Path(os.environ['GITHUB_WORKSPACE']) / 'approved-aw/plugins/faberline/skills/issue-ci/scripts'
spec = importlib.util.spec_from_file_location('fixed_grant_review_probe', ROOT / 'policy_adoption_grant_review.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
pin = json.loads((ROOT / 'policy_adoption_supply.json').read_text())['binary_sha256']
binary = Path('/usr/local/bin/claude')
assert hashlib.sha256(binary.read_bytes()).hexdigest() == pin
with tempfile.TemporaryDirectory(prefix='core-cli-help-') as tmp:
    env = dict(PATH='/usr/bin:/bin', HOME=tmp, CLAUDE_CONFIG_DIR=tmp,
               CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC='1', DISABLE_AUTOUPDATER='1')
    result = subprocess.run([str(binary), '--help'], env=env, cwd=tmp, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=False)
    required = ['--safe-mode', '--json-schema', '--tools', '--strict-mcp-config',
                '--mcp-config', '--setting-sources', '--settings', '--disable-slash-commands',
                '--no-chrome', '--no-session-persistence', '--system-prompt']
    found = {flag: flag in result.stdout for flag in required}
    print('CORE_CLAUDE_CLI_CAPABILITIES_V1 ' + json.dumps(dict(returncode=result.returncode, flags=found)), flush=True)
    assert result.returncode == 0

# Keep the exact helper's argv, empty temporary home, credential profile and schema
# checks. Capture text only in memory. Emit fixed diagnostic codes, never raw output.
def execute(args, **kwargs):
    kwargs['stderr'] = subprocess.PIPE
    result = subprocess.run(args, **kwargs)
    combined = (result.stdout + '\n' + result.stderr).lower()
    families = {
        'unknown_option': ('unknown option', 'unrecognized option', 'unexpected argument'),
        'oauth_rejected': ('oauth token has expired', 'invalid oauth token', 'authentication_error', 'not logged in'),
        'rate_limit': ('rate_limit_error', 'rate limit', 'usage limit', 'hit your limit'),
        'model_unavailable': ('model_not_found', 'model is not available', 'does not support model'),
        'context_limit': ('prompt is too long', 'context window', 'context limit', 'too many tokens'),
        'input_limit': ('input too long', 'input is too long', 'request too large', 'payload too large', 'input exceeds'),
        'long_context_access': ('extra usage', 'long context', '1m context', '1 million', 'more usage'),
        'billing': ('credit balance', 'billing', 'subscription', 'payment required'),
        'schema_rejected': ('invalid schema', 'invalid json schema', 'schema validation'),
        'transport': ('econnreset', 'enotfound', 'fetch failed', 'connection error'),
    }
    codes = [code for code, matches in families.items() if any(v in combined for v in matches)]
    envelope = {}
    try:
        outer = json.loads(result.stdout)
        for key in ('type', 'subtype'):
            value = outer.get(key)
            allowed = {'result', 'success', 'error_during_execution', 'error_max_turns',
                       'error_max_budget_usd', 'error_max_structured_output_retries'}
            envelope[key] = value if value in allowed else 'other'
        envelope['is_error'] = outer.get('is_error') is True
    except (ValueError, AttributeError):
        envelope['json'] = False
    print('CORE_CLAUDE_TRANSPORT_DIAGNOSTIC_V1 ' + json.dumps(dict(
        returncode=result.returncode, stdout_bytes=len(result.stdout.encode()),
        stderr_bytes=len(result.stderr.encode()), codes=codes, envelope=envelope,
        http_status=sorted(set(re.findall(r'(?:api error:|status(?: code)?[: =]+|http[/0-9. ]+)\s*([45][0-9]{2})\b', combined))),
        token_comparisons=re.findall(r'\b([0-9]+)\s*(?:tokens?\s*)?>\s*([0-9]+)', combined),
        token_counts=re.findall(r'\b([0-9]+)\s*(?:input |output |context )?tokens?\b', combined),
        api_error_types=[v for v in ('invalid_request_error', 'authentication_error', 'permission_error',
            'not_found_error', 'request_too_large', 'rate_limit_error', 'api_error', 'overloaded_error') if v in combined])), flush=True)
    return result

spec = importlib.util.spec_from_file_location('fixed_target_review_probe', ROOT / 'policy_adoption_target_review.py')
t = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t)
packet, trees = t.preflight(t.a.ci, 'faberline/core', 87, '57add95b4952381ad440ce4bfad826f87b307a90')
lines = [packet['issue']['title']] + [line for line in packet['issue']['body'].splitlines() if line.strip()]
schema = t.obj({'requirements': {'type': 'array', 'minItems': 1, 'items': t.obj({
    'id': t.STR, 'requirement': t.STR, 'issue_quote': {'type': 'string', 'enum': lines},
    'supporting_paths': t.STRINGS})}, 'uncertainties': t.STRINGS})
available = sorted({entry['path'] for tree in trees.values() for entry in tree
    if isinstance(entry, dict) and entry.get('type') == 'blob'
    and entry.get('mode') in ('100644', '100755') and type(entry.get('size')) is int
    and 0 <= entry['size'] <= 262144 and t.a.valid_path(entry.get('path'))})
packet = dict(packet, available_source_paths=available)
print('CORE_CLAUDE_PLANNING_PACKET_V1 ' + json.dumps(dict(
    packet_bytes=len(t.a.r.canonical(packet).encode()), schema_bytes=len(t.a.r.canonical(schema).encode()),
    source_paths=len(available), review_authority=False,
    field_bytes={key: len(t.a.r.canonical(value).encode()) for key,value in packet.items()},
    framework_rows=len(packet['shared_framework_sources']),
    framework_unique_texts=len({row['content'] for row in packet['shared_framework_sources']}),
    framework_unique_text_bytes=sum(len(v.encode()) for v in {row['content'] for row in packet['shared_framework_sources']}))), flush=True)
try:
    result = m.invoke_schema(packet,
        os.environ.get('CLAUDE_CODE_OAUTH_TOKEN'),
        t.SYSTEM + '\nPlan every issue requirement and request necessary supporting paths. Choose at most 64 distinct exact names from available_source_paths. The pinned shared framework source is already supplied separately. A source request or missing initial context is not a contract uncertainty. Report only material ambiguity that source cannot resolve. Do not make a verdict.',
        schema, execute=execute, max_packet=4*1024*1024)
    print('CORE_CLAUDE_PROBE_RESULT_V1 ' + json.dumps(dict(
        planning_returned=isinstance(result,dict), review_authority=False)), flush=True)
except RuntimeError:
    print('CORE_CLAUDE_PROBE_RESULT_V1 {"acknowledged":false,"review_authority":false}', flush=True)
