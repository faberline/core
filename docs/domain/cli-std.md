# cli-std

cli-std holds the standard agent-facing commands that every CLI in the
ecosystem ships: `llm` (offline self-documentation), `upgrade` (self-update from
the tool's GitHub releases) and `issue` (search, view, file and comment on
diagnostics-rich issues). Smaller shared pieces come with them:
- `connect`, the port-forward and token lifecycle that k8s-native service CLIs use;
- the `chainable` output check;
- the `artifact` text helpers;
- the `CliModule` subcommand registry.

The logic is parameterized by a `ToolInfo` that the calling binary fills from
its build stamps. Apart from the registry, it is clap-agnostic: each CLI keeps
its own argument parsing and calls these functions.

Six core contexts build their llm topics with it: raft-runtime, service-auth,
service-k8s, service-backup, transport-h2c and openapi-codegen. Sixteen
downstream repos depend on it: beam, cap, courier, defer, jet, keep, loom,
lumen, mamba, mesh, meter, pgpool, relay, sift, tape and vat.

**Form:** layered, with several sub-contexts in one crate (ADR D14; allowed by
`one_context_per_crate = false`) · **Depends on:** — · **Crate:** [`crates/cli-std`](../../crates/cli-std)

## Model

- **ToolInfo** — the calling binary's identity and build provenance: project,
  repo, target, version, git sha and built-at, all static strings. A binary
  builds it with the `const fn` `ToolInfo::new` and reads it through getters
  of the same names. It derives
  the issue label `app:<project>`, the release tag prefix `<project>@`, the
  release asset `<project>-<target>.tar.gz` and the binary's path inside it.
- **llm topic (v1)** — `llm::Topic`: a static help topic with an id, a summary
  and a body, built with the `const fn` `Topic::new` and read through getters
  of the same names. A `SectionedTopic` (built with the `const fn`
  `SectionedTopic::new`) is made of `TopicSection`s. Each section is either
  fixed prose or a generated section that renders at call time. `Format`
  selects Markdown or JSON output.
- **llm topic (v2)** — `llm::v2::Topic`: a `Task` paired with a `Runbook`.
  - The `Task` says when to use the topic, what it requires, reads and
    produces, its `Risk`, and its contract references.
  - The `Runbook` holds the purpose, preconditions, typed `Input`s,
    constraints, `Step`s, verification and references.
- **ProtocolDocument** — the validated v2 topics of one project, rendered under
  the `cclab.llm.v2` protocol. `json_schema` describes its JSON envelopes. A v1
  topic from a shared library can be attached to a v2 topic as
  `ProviderContent`.
- **upgrade decision** — `upgrade::Options` holds the check, pinned tag, force
  and yes flags. Comparing the installed and selected versions gives an
  `Action`: `UpToDate` or `Install`.
- **issue report** — the diagnostics block (tool identity, OS and architecture,
  and optionally the status of a running node) and the body that an issue or a
  follow-up comment carries. The options of each verb are `CreateOptions`,
  `CommentOptions` and `SearchOptions`.
- **connect role** — `connect::Role` (`Read`, `Write`, `Admin`) together with
  `TokenClaims`, which hold a subject and a role per collection (`*` grants
  every collection).
- **ChainableViolation** — the reason a command's output does not tell an agent
  its next step.
- **artifact hygiene** — release-tag normalization, operator-namespace
  substitution, trailing newlines, and the removal of `# SPEC-MANAGED:` and
  `# CODEGEN-BEGIN`/`END` ownership lines from artifacts that users build or
  apply.

## Ports

- **`CliModule`** — a subcommand that a CLI crate registers into the
  `CLI_MODULES` link-time slice (the `registry` feature). It provides a name, a
  clap command and `execute`. jet, mamba and meter implement it.
- **`RenderableTopic`** — how a v1 topic renders its sections. `Topic` and
  `SectionedTopic` implement it.

The connect, issue and upgrade use cases reach the outside through ports that
the domain defines and infrastructure implements. They are crate-internal: no
downstream code implements them.
- **`Kubectl`** (`k8s`) — reads a cluster object as JSON, and the decoded data
  of a Secret key. `KubectlCli` runs the `kubectl` binary.
- **`TrackerAccess`** (`online`) — the courier URL and the GitHub token, read
  when a verb needs them. `EnvTracker` reads the environment and
  `gh auth token`.
- **`GitHubApi`**, **`CourierApi`** and **`NodeProbe`** (`online`) — the GitHub
  issue endpoints, courier's `/v1/issues/...` endpoints, and the status of a
  running node.
- **`ReleaseSource`** (`online`) — the tool's GitHub releases and their asset
  downloads. `HttpClient` implements it and the three ports above.
- **`Confirm`** (`online`) — the yes-or-no question before a verb changes
  anything. `TerminalPrompt` asks on the terminal.
- **`SelfInstall`** (`online`) — replaces the running binary. `SelfReplace`
  writes a sibling file and renames it over the executable.

The port errors (`KubectlError`, `TokenRegistryError`, `RemoteError`,
`PromptError`, `InstallError`) keep the messages of the `anyhow` context they
replace; the public entry points still return `anyhow::Result`.

The composition root, `src/app/`, holds the public entry points
(`issue::{create, comment, search, view}`, `upgrade::run`,
`connect::{resolve_token, resolve_cr_tokens_secret}`): each builds the
adapters and calls its use case. Their signatures are unchanged.

## Invariants

- **Version selection:**
  - A pin (`X.Y.Z` or `<project>@X.Y.Z`) must match a release exactly.
  - Without a pin, the highest stable (non-prerelease) release wins.
  - Tags without the tool's prefix, and tags that are not valid semver, are
    ignored.
- **Installs:**
  - A binary is installed only when the selected version differs from the
    installed one, or the run is forced.
  - The tarball must match its `.sha256` sidecar. The comparison ignores case
    and surrounding whitespace. On a mismatch the run aborts and leaves the
    existing binary unchanged.
- **Issue verbs:**
  - `issue create` always adds the `app:<project>` and `type:report` labels to
    the caller's labels.
  - `issue comment` reopens the issue before it comments.
  - Without a GitHub token, both fall back to printing a pre-filled URL or the
    text of the comment. The token is read from `GH_TOKEN`, then
    `GITHUB_TOKEN`, then `gh auth token`.
- **Courier proxy mode:** when `AXIOM_COURIER_URL` is set and not blank, the
  four issue verbs go through courier's `/v1/issues/...` endpoints, using
  `AXIOM_COURIER_TOKEN`. When it is unset or blank, they call GitHub directly,
  and that path is byte-identical to the behaviour before courier existed.
- **ProtocolDocument validity:**
  - The project name is not empty.
  - Task ids and topics are not empty and are unique.
  - Each step has either a fully bound `command` or a `command_template`, never
    both. A fully bound command has no inputs and no `<` or `{`.
  - A `command_template`'s placeholders exactly match the step's typed inputs.
  - A provider needs an existing topic and a non-empty id and body, and its id
    is unique within that topic.
- **v2 JSON** keeps the v1 compatibility fields `topic` and `markdown`, and adds
  `protocol` together with the task list (for the outline) or the task and
  runbook (for a single topic).
- **v1 topics:** `assert_topics_render` requires every topic to render
  non-empty, and every generated section to be non-empty, with ids that are
  unique across the whole set.
- **Tokens and the registry:**
  - The connect role order is `Admin` ⊇ `Write` ⊇ `Read`.
  - Token selection uses the grant for the collection, and falls back to `*`.
  - A token registry is read in either shape: namespaced
    (`{tokens, identities}`) or flat. Only `tokens` are presentable.
  - The registry reading must match `service_auth::Registry::parse`, and a
    shared test pins the two together. The copy exists because service-auth
    depends on cli-std, not the reverse.
- **Chainable output:** output is chainable when it is an `aw.cli.v1` envelope
  (`invoke.command`, `next.command`, or `completion.workflow_complete: true`),
  or when it uses the lightweight form: a top-level JSON `next` string, or a
  trailing `next: <cmd>` or `next: done` line.

## Published language

The v1 llm help-topic model lives in the application layer, because six other
core contexts build their llm topics with it (ADR D19). The model is `Topic`,
`SectionedTopic`, `TopicSection`, `render_sectioned` and
`assert_topics_render`.

Downstream CLIs use the whole public API:
- the crate-root `ToolInfo`;
- the public modules `issue`, `upgrade`, `llm`, `llm::v2`, `connect`,
  `chainable`, `artifact`, `registry` and `report_issue`. They keep their
  paths (`src/api/`) because the root does not re-export their names.

jet, mamba and meter register into `cli_std::registry::CLI_MODULES`, a
`linkme` distributed slice, through that exact path with
`#[distributed_slice(...)]`. The `registry` module must keep that path working.

`report_issue` is a deprecated alias of `issue`; cap and mamba still use it.

## Exceptions and debts

- **Checker exceptions:** none. The upgrade version rules use `semver`, which
  is not on the domain allowlist, so they sit in the application layer and need
  no exception.
- **Tracked for P2:**
  - Public fields built with struct literals (ADR D2):
    - `upgrade::Options` and the issue option structs, in the CLIs of beam,
      cap, courier, defer, jet, keep, loom, lumen, mamba, mesh, pgpool, relay,
      sift, tape and vat;
    - the v2 `Topic`, `Task`, `Runbook`, `Step` and `Input`, in lumen.
  - `TopicSection` is an enum and keeps its public variants; tape builds its
    `Generated` variant with named fields.
  - `anyhow` in public signatures, including the `CliModule::execute` port
    that jet, mamba and meter implement (ADR D4).
  - The `issue` and `upgrade` handlers print their results directly. Moving the
    output to the interfaces layer must keep stdout byte-identical, so this is
    a behaviour risk.
