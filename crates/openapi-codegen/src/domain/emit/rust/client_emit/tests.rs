use super::*;
use crate::build_type_map;
use crate::ir::openapi::Spec;
use crate::ir::operations;

fn render() -> String {
    let s: Spec = serde_json::from_str(r##"{"paths":{}}"##).unwrap();
    let tm = build_type_map(&s);
    emit(&operations::build(&s), &tm, None)
}

#[test]
fn a_private_ca_replaces_the_public_roots_rather_than_joining_them() {
    let out = render();
    assert!(
        out.contains(".tls_built_in_root_certs(false)"),
        "keeping the public roots means any public CA can still vouch for this \
         name, which is the thing a private trust domain exists to prevent"
    );
    assert!(out.contains(".add_root_certificate(anchor)"));
}

#[test]
fn the_verified_name_must_be_the_addressed_name() {
    let out = render();
    assert!(out.contains("if addressed != trust.server_name {"));
    assert!(out.contains("fn addressed_host(base_url: &str) -> &str {"));
}

#[test]
fn no_generated_client_offers_to_skip_verification() {
    let out = render();
    for weakening in [
        "danger_accept_invalid_certs",
        "danger_accept_invalid_hostnames",
        "insecure",
        "skip_verify",
    ] {
        assert!(
            !out.contains(weakening),
            "generated clients must not ship {weakening}: the fix for an unverifiable \
             server is the anchor or the name, not the check"
        );
    }
}
