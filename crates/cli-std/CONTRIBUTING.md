# Contributing to cli-std

## Brief

How to change `crates/cli-std`. What it promises and the work roots it owns live
in [README.md](README.md); repository-wide authoring and verification rules
live in the root [CONTRIBUTING.md](../../CONTRIBUTING.md).

There is no phase ladder here. `leg.leg_root` resolves `apps/<project>` and
nothing else, so a change to this crate is authored and committed directly
rather than driven through `e2e -> impl`.

## CLI llm convention

The convention for [Standard Agent CLI Commands](README.md#standard-agent-cli-commands):

1. Fact-bearing content is a `TopicSection::Generated` section or it does not appear.
2. Hand-written prose must not restate machine-derivable facts.
3. `llms.txt` is generated output, checked by `assert_snapshot_matches`, never hand-edited.
4. Each CLI's tests call `assert_topics_render` and `assert_snapshot_matches`.

## Verification

| Gate | Command |
|---|---|
| unit + colocated tests | `cargo test -p cli-std` |
