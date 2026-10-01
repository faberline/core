use super::TcpHandlerError;

#[test]
fn other_keeps_the_wrapped_message_and_type() {
    let error = TcpHandlerError::other(std::io::Error::other("connection reset"));
    assert_eq!(error.to_string(), "connection reset");
    let TcpHandlerError::Other(inner) = error;
    assert!(inner.downcast_ref::<std::io::Error>().is_some());
}

#[test]
fn a_message_logs_as_itself() {
    let error = TcpHandlerError::other(format!("tls handshake failed: {}", "bad record"));
    assert_eq!(error.to_string(), "tls handshake failed: bad record");
}

#[test]
fn an_anyhow_error_logs_the_text_it_logged_before() {
    // `serve` logs `%error`. An anyhow body wrapped with `other` must show
    // what the anyhow error itself showed: its outermost message.
    let anyhow_error =
        anyhow::Error::new(std::io::Error::other("broken pipe")).context("write startup reply");
    let before = format!("{anyhow_error}");
    let error = TcpHandlerError::other(anyhow_error);
    assert_eq!(format!("{error}"), before);
    assert_eq!(before, "write startup reply");
}
