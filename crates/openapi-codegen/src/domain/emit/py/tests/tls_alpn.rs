use super::*;

#[test]
fn generated_python_h2_runtime_negotiates_tls_alpn_h2() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let cert = dir.join("cert.pem");
    let key = dir.join("key.pem");
    let Ok(output) = Command::new("openssl")
        .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout"])
        .arg(&key)
        .args(["-out"])
        .arg(&cert)
        .args(["-subj", "/CN=localhost", "-days", "1"])
        .output()
    else {
        let _ = fs::remove_dir_all(&dir);
        return;
    };
    if !output.status.success() {
        let _ = fs::remove_dir_all(&dir);
        return;
    }

    let script = format!(
        r#"
import asyncio
import socket
import ssl
import sys
import threading
import time
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import AsyncH2CClient, H2CClient

def write_frame(conn, kind, flags, stream_id, payload):
    conn.sendall(len(payload).to_bytes(3, "big") + bytes([kind, flags]) + (stream_id & 0x7fffffff).to_bytes(4, "big") + payload)

def read_exact(conn, n):
    out = bytearray()
    while len(out) < n:
        chunk = conn.recv(n - len(out))
        if not chunk:
            raise RuntimeError("connection closed")
        out.extend(chunk)
    return bytes(out)

def read_frame(conn):
    head = read_exact(conn, 9)
    size = int.from_bytes(head[:3], "big")
    return head[3], head[4], int.from_bytes(head[5:9], "big") & 0x7fffffff, read_exact(conn, size)

def enc_int(value, prefix_bits, prefix=0):
    max_prefix = (1 << prefix_bits) - 1
    if value < max_prefix:
        return bytes([prefix | value])
    out = bytearray([prefix | max_prefix])
    value -= max_prefix
    while value >= 128:
        out.append((value % 128) | 0x80)
        value //= 128
    out.append(value)
    return bytes(out)

def enc_str(value):
    raw = value.encode("utf-8")
    return enc_int(len(raw), 7, 0) + raw

def literal(name, value):
    return b"\x00" + enc_str(name) + enc_str(value)

def response_headers(length):
    return b"\x88" + literal("content-type", "application/json") + literal("content-length", str(length))

def serve_once(label):
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen(1)
    port = listener.getsockname()[1]
    def run():
        raw, _ = listener.accept()
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.load_cert_chain({cert:?}, {key:?})
        ctx.set_alpn_protocols(["h2"])
        conn = ctx.wrap_socket(raw, server_side=True)
        assert conn.selected_alpn_protocol() == "h2"
        assert read_exact(conn, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
        write_frame(conn, 4, 0, 0, b"")
        while True:
            kind, flags, stream_id, payload = read_frame(conn)
            if kind == 4 and not (flags & 0x1):
                write_frame(conn, 4, 0x1, 0, b"")
            elif kind == 1:
                body = ("{{\"alpn\":\"h2\",\"client\":\"%s\"}}" % label).encode("utf-8")
                write_frame(conn, 1, 0x4, stream_id, response_headers(len(body)))
                write_frame(conn, 0, 0x1, stream_id, body)
                time.sleep(0.1)
                conn.close()
                listener.close()
                return
    thread = threading.Thread(target=run)
    thread.start()
    return port, thread

client_ctx = ssl.create_default_context()
client_ctx.check_hostname = False
client_ctx.verify_mode = ssl.CERT_NONE
port, thread = serve_once("sync")
client = H2CClient(timeout=5, ssl_context=client_ctx)
sync_payload = client.get(f"https://127.0.0.1:{{port}}/alpn").json()
client.close()
thread.join()
assert sync_payload == {{"alpn": "h2", "client": "sync"}}

async def main():
    async_ctx = ssl.create_default_context()
    async_ctx.check_hostname = False
    async_ctx.verify_mode = ssl.CERT_NONE
    port, thread = serve_once("async")
    async with AsyncH2CClient(timeout=5, ssl_context=async_ctx) as client:
        response = await client.get(f"https://127.0.0.1:{{port}}/alpn")
        payload = response.json()
    thread.join()
    assert payload == {{"alpn": "h2", "client": "async"}}

asyncio.run(main())
"#,
        dir = dir.display().to_string(),
        cert = cert.display().to_string(),
        key = key.display().to_string(),
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python TLS ALPN h2 smoke");
    assert!(
        output.status.success(),
        "generated Python TLS ALPN h2 smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}
