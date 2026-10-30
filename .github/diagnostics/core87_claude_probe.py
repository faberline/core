"""Read-only model transport probe. This cannot approve or merge a PR."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile

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
        stderr_bytes=len(result.stderr.encode()), codes=codes, envelope=envelope)), flush=True)
    return result

schema = {'type': 'object', 'additionalProperties': False, 'required': ['ack'],
          'properties': {'ack': {'type': 'string', 'enum': ['diagnostic-only']}}}
try:
    result = m.invoke_schema({'diagnostic': 'Transport probe only. No code review or authorization.'},
        os.environ.get('CLAUDE_CODE_OAUTH_TOKEN'),
        'This is a transport diagnostic. Return the requested acknowledgement schema. No tools or code review.',
        schema, execute=execute)
    print('CORE_CLAUDE_PROBE_RESULT_V1 ' + json.dumps(dict(
        acknowledged=result == {'ack': 'diagnostic-only'}, review_authority=False)), flush=True)
except RuntimeError:
    print('CORE_CLAUDE_PROBE_RESULT_V1 {"acknowledged":false,"review_authority":false}', flush=True)
