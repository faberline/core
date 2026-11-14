//! A reload with new material keeps an open peer connection alive (issue 355,
//! point 7 of the 1.0.0 security gate, faberline/aw#340).
//!
//! The first test proves that an already open connection survives a reload,
//! and that the next handshake uses the new material. The second test is the
//! negative control: after a reload to a new authority and retirement of the
//! old one, a leaf from the retired authority is refused.

use crate::support;

use std::sync::Arc;

use peer_tls::material::MaterialPem;
use peer_tls::reload::{MaterialSource, MemoryMaterialSource, ReloadableTls, TlsRuntimeProfile};
use support::{authority, bundle, handshake, open, Authority, LeafBuilder};

const MEMBER: &str = "lumen-0.lumen-peer.axiom.svc.cluster.local";
const SPIFFE: &str = "spiffe://axiom/ns/axiom/instance/lumen-0";
const PEER: &str = "lumen-1.lumen-peer.axiom.svc.cluster.local";
const PEER_SPIFFE: &str = "spiffe://axiom/ns/axiom/instance/lumen-1";

fn material(ca: &Authority, trust: String, dns: &str, spiffe: &str) -> MaterialPem {
    let leaf = LeafBuilder::new().dns(&[dns]).spiffe(&[spiffe]).issue(ca);
    MaterialPem::new(leaf.cert_pem, leaf.key_pem, trust)
}

fn member(ca: &Authority) -> (ReloadableTls, Arc<MemoryMaterialSource>) {
    let source = Arc::new(MemoryMaterialSource::new(material(
        ca,
        ca.pem(),
        MEMBER,
        SPIFFE,
    )));
    let tls = ReloadableTls::required(
        TlsRuntimeProfile::peer([MEMBER.to_string()], [SPIFFE.to_string()]),
        source.clone() as Arc<dyn MaterialSource>,
    )
    .expect("startup material");
    (tls, source)
}

fn dialer(ca: &Authority, trusts: &[&Authority]) -> Arc<rustls::ClientConfig> {
    let source = Arc::new(MemoryMaterialSource::new(material(
        ca,
        bundle(trusts),
        PEER,
        PEER_SPIFFE,
    )));
    ReloadableTls::required(
        TlsRuntimeProfile::peer([PEER.to_string()], [PEER_SPIFFE.to_string()]),
        source as Arc<dyn MaterialSource>,
    )
    .expect("the dialing peer's material")
    .client_config()
    .unwrap()
}

#[tokio::test]
async fn reload_with_new_material_does_not_drop_an_open_connection() {
    let ca = authority("axiom-peer-ca");
    let (tls, source) = member(&ca);

    let mut session = open(tls.server_config().unwrap(), dialer(&ca, &[&ca]), MEMBER)
        .await
        .expect("the first connection");
    assert_eq!(
        session.roundtrip(b"before-reload").await.unwrap(),
        b"before-reload"
    );
    let before = session.server_leaf_sha256.clone();

    source.set(material(&ca, ca.pem(), MEMBER, SPIFFE));
    tls.reload().expect("the renewed leaf activates");
    assert_eq!(tls.generation(), 2);
    assert_ne!(tls.fingerprint().unwrap(), before, "new material, new leaf");

    // The connection that was already up still works after the reload.
    assert_eq!(
        session.roundtrip(b"after-reload").await.unwrap(),
        b"after-reload"
    );

    // A handshake after the reload uses the new material.
    let second = open(tls.server_config().unwrap(), dialer(&ca, &[&ca]), MEMBER)
        .await
        .expect("a handshake after the reload");
    assert_eq!(second.server_leaf_sha256, tls.fingerprint().unwrap());
    assert_ne!(second.server_leaf_sha256, before);

    session.close().await;
    second.close().await;
}

#[tokio::test]
async fn reload_with_new_material_refuses_a_new_connection_from_the_retired_authority() {
    let outgoing = authority("axiom-peer-ca-gen1");
    let incoming = authority("axiom-peer-ca-gen2");
    let (tls, source) = member(&outgoing);

    // Before the reload the old authority is accepted. This is what makes the
    // refusal afterwards meaningful.
    let before = handshake(
        tls.server_config().unwrap(),
        dialer(&outgoing, &[&outgoing, &incoming]),
        MEMBER,
    )
    .await;
    assert!(
        before.accepted(),
        "the outgoing authority must be accepted before the reload: {:?}",
        before.server
    );

    source.set(material(&incoming, incoming.pem(), MEMBER, SPIFFE));
    tls.reload().expect("the new authority activates");
    assert!(tls.retire_previous_trust(tls.generation()));

    let after = handshake(
        tls.server_config().unwrap(),
        dialer(&outgoing, &[&outgoing, &incoming]),
        MEMBER,
    )
    .await;
    assert!(
        after.server.is_err(),
        "a leaf from the retired authority must be refused after the reload"
    );

    let current = handshake(
        tls.server_config().unwrap(),
        dialer(&incoming, &[&incoming]),
        MEMBER,
    )
    .await;
    assert!(
        current.accepted(),
        "a leaf from the new authority must be accepted: {:?}",
        current.server
    );
}
