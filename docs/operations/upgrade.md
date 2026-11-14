# Upgrade and rollback

This page moves an adopter workspace from the newest `0.x` tag of core to
`1.0.0`, and back to the old tag if the upgrade must be undone. The examples use
`v0.6.0` as the old tag; replace it with the tag your workspace pins.

## Before you start

Find the newest `0.x` tag in a clone of core with
`git tag --list 'v0.*' | sort -V | tail -1`. Pin one tag for every core crate in
your workspace, as described in [README.md](README.md#how-downstream-repos-consume-core).
Two tags in one workspace give two copies of every shared type, and they do not
unify.

Read the release notes page `release-notes-<version>.md` and the pages in
[`../migration/`](../migration/) for every version between the old tag and the
new one. The migration pages are `ddd-p1.md`, `ddd-p1-paths.md`, `ddd-p2.md`,
`ddd-p2-paths.md`, `ddd-p2-downstream.md` and `ddd-p2-validation.md`.

Check that the workspace pins one tag for every core crate. This command prints
nothing when every `tag = ` line names `v1.0.0`:

<!-- tested -->
```sh
cat > Cargo.toml <<'EOF'
[dependencies]
service-k8s = { git = "https://github.com/faberline/core", tag = "v1.0.0" }
server-http = { git = "https://github.com/faberline/core", tag = "v1.0.0" }
EOF
test -z "$(grep 'tag = ' Cargo.toml | grep -v 'tag = "v1.0.0"')"
```

## Upgrade to 1.0.0

1. Change every core dependency to `tag = "v1.0.0"` in one commit. This block
   makes that edit to a sample `Cargo.toml` and checks the result:

<!-- tested -->
```sh
cat > Cargo.toml <<'EOF'
[dependencies]
service-k8s = { git = "https://github.com/faberline/core", tag = "v0.6.0" }
server-http = { git = "https://github.com/faberline/core", tag = "v0.6.0" }
EOF
sed 's/tag = "v0.6.0"/tag = "v1.0.0"/' Cargo.toml > Cargo.toml.new
mv Cargo.toml.new Cargo.toml
test "$(grep -c 'tag = "v1.0.0"' Cargo.toml)" = 2
! grep -q 'v0.6.0' Cargo.toml
```

2. Read the migration pages and do each migration step in the same commit or a
   commit after it.
3. Run `cargo check --workspace --all-targets`, `cargo test --workspace` and the
   release-candidate checks of your service.

`v1.0.0` is the target tag. If the tag does not exist yet, the blocks on this
page still run, because they edit only a local file.

## Roll back

To go back to the old tag, pin the old tag again in one commit and run the same
checks as in the upgrade. This block makes that edit and checks it:

<!-- tested -->
```sh
cat > Cargo.toml <<'EOF'
[dependencies]
service-k8s = { git = "https://github.com/faberline/core", tag = "v1.0.0" }
server-http = { git = "https://github.com/faberline/core", tag = "v1.0.0" }
EOF
sed 's/tag = "v1.0.0"/tag = "v0.6.0"/' Cargo.toml > Cargo.toml.new
mv Cargo.toml.new Cargo.toml
test "$(grep -c 'tag = "v0.6.0"' Cargo.toml)" = 2
! grep -q 'v1.0.0' Cargo.toml
```

Tags are never moved or reused, so the old tag always builds the old code.

A rollback does not undo data. A change that makes a node fail to read data
that an older tag wrote is a breaking change (see the rules in
[README.md](README.md)). Data that `1.0.0` wrote may therefore not be readable
by the old tag. The safe rollback restores the data from a backup taken before
the upgrade; see [backup-restore.md](backup-restore.md) for what is available.

## Commands that run without a cluster

The shell blocks on this page that follow a tested marker are run by
`ci/test-upgrade-doc-commands.sh`. Each block runs in its own empty temporary
directory and needs no cluster, no network and no `sudo`.
