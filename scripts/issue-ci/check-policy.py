#!/usr/bin/env python3
"""Validate explicit full qualification or an explicitly held legacy policy."""
import importlib.util
from pathlib import Path
import sys


def load_controller():
    path = Path(__file__).with_name('handoff.py')
    spec = importlib.util.spec_from_file_location('issue_ci_handoff', path)
    controller = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(controller)
    return controller


def check(path):
    controller = load_controller()
    names = ('load_config', 'evaluate_policy', 'validate_adoption', 'qualification_module')
    if not all(callable(getattr(controller, name, None)) for name in names):
        raise RuntimeError('reviewed qualification-aware controller required')
    policy = controller.load_config(path)
    controller.validate_adoption(policy)
    if policy['lifecycle_mode'] == 'failure-only':
        if not policy['qualification_blockers']:
            raise RuntimeError('failure-only requires explicit nonempty qualification blockers')
    else:
        if policy['qualification_blockers'] or 'qualification' not in policy:
            raise RuntimeError('full qualification requires the complete approved contract and no unresolved blockers')
        controller.qualification_module().validate(policy)
    return policy


if __name__ == '__main__':
    check(sys.argv[1])
