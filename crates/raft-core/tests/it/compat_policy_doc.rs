//! The "Compatibility and pinning" rule in `docs/operations/README.md` exists and
//! keeps its rule headings, so a later edit cannot drop it silently (#227).

const OPERATIONS_README: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/operations/README.md"
));

const REQUIRED_HEADINGS: [&str; 6] = [
    "## Compatibility and pinning",
    "### Which tag a service pins",
    "### Moving to a new tag",
    "### What a breaking change is",
    "### What a breaking change needs",
    "### Renames of pinned tests",
];

/// Each required string that is not a whole line of `doc` (trimmed lines).
fn missing_headings(doc: &str, required: &[&str]) -> Vec<String> {
    required
        .iter()
        .filter(|want| !doc.lines().any(|line| line.trim() == **want))
        .map(|want| want.to_string())
        .collect()
}

#[test]
fn operations_readme_has_the_compatibility_and_pinning_section() {
    assert_eq!(
        missing_headings(OPERATIONS_README, &REQUIRED_HEADINGS),
        Vec::<String>::new()
    );
}

#[test]
fn the_heading_check_reports_a_dropped_heading() {
    let doc = OPERATIONS_README
        .lines()
        .filter(|l| l.trim() != "### Moving to a new tag")
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        missing_headings(&doc, &REQUIRED_HEADINGS),
        vec!["### Moving to a new tag".to_string()]
    );
}

#[test]
fn the_pinning_rule_names_the_five_golden_crates() {
    for name in [
        "raft-core",
        "raft-runtime",
        "service-auth",
        "service-backup",
        "service-k8s",
        "wire_golden",
    ] {
        assert!(
            OPERATIONS_README.contains(name),
            "docs/operations/README.md must name {name}"
        );
    }
}
