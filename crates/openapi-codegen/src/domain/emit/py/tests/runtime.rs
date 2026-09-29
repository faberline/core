use super::*;

#[test]
fn runtime_exposes_unary_and_bidi_surfaces() {
    let out = generate(SPEC, &opts()).unwrap();
    let runtime = file(&out, "h2c_runtime.py");
    assert!(runtime.contains("import asyncio"));
    assert!(runtime.contains("import ssl"));
    assert!(runtime.contains("class H2CClient:"));
    assert!(runtime.contains("class H2CConnection:"));
    assert!(runtime.contains("TLS ALPN h2"));
    assert!(runtime.contains("selected_alpn_protocol()"));
    assert!(runtime.contains("def request("));
    assert!(runtime.contains("def get("));
    assert!(runtime.contains("def stream("));
    assert!(runtime.contains("max_connections_per_origin"));
    assert!(runtime.contains("_DEFAULT_MAX_CONNECTIONS = 128"));
    assert!(runtime.contains("_DEFAULT_MAX_KEEPALIVE_CONNECTIONS = 16"));
    assert!(runtime.contains("def recommended_h2c_connections("));
    assert!(runtime.contains("target_concurrency: Optional[int] = None"));
    assert!(runtime.contains("max_in_flight_per_origin"));
    assert!(runtime.contains("pool_timeout"));
    assert!(runtime.contains("threading.BoundedSemaphore"));
    assert!(runtime.contains("asyncio.BoundedSemaphore"));
    assert!(runtime
        .contains("default_headers: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None"));
    assert!(runtime.contains("self._default_headers[\"Authorization\"] = f\"Bearer {auth_token}\""));
    assert!(runtime.contains("_DEFAULT_TIMEOUT = 5.0"));
    assert!(runtime.contains("_DEFAULT_MAX_RESPONSE_BYTES = 64 * 1024 * 1024"));
    assert!(runtime.contains("class H2CStream:"));
    assert!(runtime.contains("def send_data("));
    assert!(runtime.contains("def iter_json_lines("));
    assert!(runtime.contains("class AsyncH2CClient:"));
    assert!(runtime.contains("class AsyncH2CConnection:"));
    assert!(runtime.contains("class AsyncH2CStream:"));
    assert!(runtime.contains("class _AsyncH2CStreamContext:"));
    assert!(runtime.contains("async def request("));
    assert!(runtime.contains("async def iter_json_lines("));
    assert!(runtime.contains("def _decode_huffman("));
    assert!(runtime.contains("import socket"));
    assert!(!runtime.contains("from h2"));
    assert!(!runtime.contains("pip install h2"));
}

#[test]
fn generated_python_hpack_encoder_uses_static_table_indexes() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let script = format!(
        r#"
import sys
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import _encode_headers
encoded = _encode_headers([
    (":method", "GET"),
    (":scheme", "http"),
    (":path", "/"),
    ("accept", "application/json"),
    ("content-type", "application/json"),
])
assert encoded[:3] == b"\x82\x86\x84", encoded
assert b":method" not in encoded
assert b":scheme" not in encoded
assert b":path" not in encoded
assert b"accept" not in encoded
assert b"content-type" not in encoded
assert len(encoded) < 50, encoded
"#,
        dir = dir.display().to_string(),
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python HPACK static table smoke");
    assert!(
        output.status.success(),
        "generated Python HPACK static table smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generated_python_query_params_encode_bool_wire_values() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let script = format!(
        r#"
import sys
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import _url_with_params

url = _url_with_params("http://127.0.0.1/items?existing=1", {{"force": True, "dry": False, "skip": None}})
assert url == "http://127.0.0.1/items?existing=1&force=true&dry=false", url

url = _url_with_params("http://127.0.0.1/items", [("flag", [True, False])])
assert url == "http://127.0.0.1/items?flag=true&flag=false", url
"#,
        dir = dir.display().to_string(),
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python query param encoding smoke");
    assert!(
        output.status.success(),
        "generated Python query param encoding smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generated_python_h2_runtime_rejects_unsafe_protocol_inputs() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let script = format!(
        r#"
import sys
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import (
    H2CConnection,
    H2CProtocolError,
    H2CStream,
    _FLAG_END_HEADERS,
    _HpackDecoder,
    _MAX_FLOW_CONTROL_WINDOW,
    _MAX_HEADER_BLOCK,
    _MAX_INBOUND_FRAME,
    _encode_int,
    _header_pairs,
    _validate_method,
)

def must_reject(callback):
    try:
        callback()
    except H2CProtocolError:
        return
    except Exception as exc:
        raise AssertionError(f"expected H2CProtocolError, got {{type(exc).__name__}}: {{exc}}") from exc
    raise AssertionError("expected H2CProtocolError")

must_reject(lambda: _header_pairs({{"bad name": "x"}}))
must_reject(lambda: _header_pairs({{"x-test": "ok\r\nbad: yes"}}))
must_reject(lambda: _validate_method("GET /bad"))
must_reject(lambda: _HpackDecoder().decode(_encode_int(4097, 5, 0x20)))

conn = H2CConnection("http", "127.0.0.1", 1)
must_reject(lambda: conn._headers_payload(_FLAG_END_HEADERS, 1, b"x" * (_MAX_HEADER_BLOCK + 1)))
must_reject(lambda: conn._handle_settings(0, b"\x00\x04" + (_MAX_FLOW_CONTROL_WINDOW + 1).to_bytes(4, "big")))
must_reject(lambda: conn._handle_settings(0, b"\x00\x05\x00\x00\x00\x01"))

conn = H2CConnection("http", "127.0.0.1", 1)
conn._conn_send_window = _MAX_FLOW_CONTROL_WINDOW
must_reject(lambda: conn._handle_window_update(0, (1).to_bytes(4, "big")))

limited = H2CConnection("http", "127.0.0.1", 1, max_response_bytes=1)
stream = H2CStream(limited, 1)
stream.status_code = 200
stream._chunks.append(b"ok")
stream._response_ended = True
must_reject(stream.read_response)

stream = H2CStream(H2CConnection("http", "127.0.0.1", 1), 1)
must_reject(lambda: stream._handle_headers([("bad name", "x")]))
must_reject(lambda: stream._handle_headers([("x-test", "ok\nbad")]))

class BigFrameSock:
    def __init__(self):
        self.buf = (_MAX_INBOUND_FRAME + 1).to_bytes(3, "big") + b"\x00\x00\x00\x00\x00\x01"

    def recv(self, n):
        out = self.buf[:n]
        self.buf = self.buf[n:]
        return out

conn = H2CConnection("http", "127.0.0.1", 1)
conn._sock = BigFrameSock()
must_reject(conn._read_frame)
"#,
        dir = dir.display().to_string(),
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python h2 security smoke");
    assert!(
        output.status.success(),
        "generated Python h2 security smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}
