# Upgrade and rollback

This page moves an adopter workspace from the newest `0.x` tag to `1.0.0`, and back. The examples use `v0.6.0` as the old tag and `v1.0.0` as the new one; replace `v0.6.0` with the tag your workspace pins.

## Before you start

- Find the newest `0.x` tag of [faberline/core](https://github.com/faberline/core) with `git tag --list 'v0.*' | sort -V | tail -1`. That is the tag you upgrade from.
- Pin one tag for every core crate. Two tags mean two copies of every shared type (see [How downstream repos consume core](README.md#how-downstream-repos-consume-core)). The check below proves that a file pins one tag:

<!-- tested -->
```sh
cat > Cargo.toml <<'EOF'
[dependencies]
service-k8s = { git = "https://github.com/faberline/core", tag = "v1.0.0" }
server-http = { git = "https://github.com/faberline/core", tag = "v1.0.0" }
EOF
test "$(grep -o 'tag = "[^"]*"' Cargo.toml | sort -u | wc -l)" = 1
```

- Read the release notes for each version between the old tag and the new one (`release-notes-<version>.md` in this folder), and every page in [`../migration/`](../migration/): `ddd-p1.md`, `ddd-p1-paths.md`, `ddd-p2.md`, `ddd-p2-paths.md`, `ddd-p2-downstream.md` and `ddd-p2-validation.md`.

## Upgrade to 1.0.0

1. Change every core dependency to `tag = "v1.0.0"` in one commit. The commands below make that edit on a sample file and check it:

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

   `v1.0.0` is the target tag. The commands above edit only a local sample file, so they run even before the tag is published.

2. Read the migration pages and do each step they name.
3. Run `cargo check --workspace --all-targets`, `cargo test --workspace` and the release-candidate checks of your service.

## Roll back

Roll back by pinning the old tag again in one commit. The commands below swap the tags of the sample file and check the result:

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

Then run the same checks as in step 3 of the upgrade.

Tags are never moved or reused (see [the release rules](README.md)), so the old tag always builds the old code.

Data is the part a tag change cannot undo. Data that `1.0.0` wrote may not be readable by a `0.x` tag. The safe rollback is to restore the data from a backup that was taken before the upgrade; see [backup-restore.md](backup-restore.md). Take that backup before you start the upgrade.

## Commands that run without a cluster

Every block that follows the tested marker on this page is run by `ci/test-upgrade-doc-commands.sh` in its own empty temporary directory. The blocks use only `cat`, `sed`, `grep`, `mv`, `mkdir` and `test`, and they need no network and no cluster.
