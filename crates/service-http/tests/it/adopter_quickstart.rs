//! The adopter quickstart stays in step with the example CI compiles, and the
//! two READMEs keep linking it.

#[test]
fn quickstart_rust_block_is_the_compiled_example() {
    let doc = include_str!("../../../../docs/operations/adopter-quickstart.md");
    let example = include_str!("../../examples/adopter_quickstart.rs");
    assert!(
        doc.contains(example.trim_end()),
        "docs/operations/adopter-quickstart.md must contain the text of crates/service-http/examples/adopter_quickstart.rs byte for byte"
    );
}

#[test]
fn quickstart_doc_has_the_numbered_steps() {
    let doc = include_str!("../../../../docs/operations/adopter-quickstart.md");
    for needle in [
        "## Step 1",
        "## Step 2",
        "## Step 3",
        "## Step 4",
        "## Step 5",
        "## Compatibility",
        "## Runbook",
        "service-http",
        "server-http",
        "service-auth",
        "raft-runtime",
        "service-backup",
        "service-k8s",
        "tag = \"v0.5.0\"",
    ] {
        assert!(
            doc.contains(needle),
            "docs/operations/adopter-quickstart.md is missing {needle:?}"
        );
    }
}

#[test]
fn quickstart_names_the_first_step_and_the_three_replica_path() {
    let doc = include_str!("../../../../docs/operations/adopter-quickstart.md");
    for needle in [
        "cargo new",
        "serve_h2c_with_lifecycle",
        "ReplicaHostBuilder",
        "build_secure",
        "runtime.host",
        "SHARD_COUNT",
        "REPLICAS_PER_SHARD=3",
        "VOTER_COUNT=3",
        "POD_NAME",
    ] {
        assert!(
            doc.contains(needle),
            "docs/operations/adopter-quickstart.md is missing {needle:?}"
        );
    }
}

#[test]
fn readme_links_the_quickstart() {
    let readme = include_str!("../../../../README.md");
    assert!(
        readme.contains("docs/operations/adopter-quickstart.md"),
        "README.md must link docs/operations/adopter-quickstart.md"
    );
}

#[test]
fn docs_readme_links_the_quickstart() {
    let readme = include_str!("../../../../docs/README.md");
    assert!(
        readme.contains("operations/adopter-quickstart.md"),
        "docs/README.md must link operations/adopter-quickstart.md"
    );
}
