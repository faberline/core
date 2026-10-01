use super::*;

#[test]
fn generated_python_h2c_runtime_reuses_one_connection_for_sequential_requests() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let (base_url, server) = spawn_h2c_sequential_server(2);
    let script = format!(
        r#"
import sys
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import H2CClient
client = H2CClient(timeout=5)
first = client.get({base_url:?} + "/first").json()
second = client.get({base_url:?} + "/second").json()
client.close()
assert first["stream"] == 1
assert second["stream"] == 3
"#,
        dir = dir.display().to_string(),
        base_url = base_url,
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python sequential h2c reuse smoke");
    let server_result = server.join();
    assert!(
        output.status.success(),
        "generated Python sequential h2c reuse smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server_result.expect("h2c sequential server panicked");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generated_python_h2c_runtime_reused_connection_perf_guard() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let request_count = 128_usize;
    let (base_url, server) = spawn_h2c_sequential_server(request_count);
    let script = format!(
        r#"
import sys
import time
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import H2CClient
client = H2CClient(timeout=5)
started = time.perf_counter()
for index in range({request_count}):
    payload = client.get({base_url:?} + f"/perf/{{index}}").json()
    assert payload["stream"] == index * 2 + 1
elapsed = time.perf_counter() - started
client.close()
print(f"h2c_generated_sync_reused_connection seconds={{elapsed:.6f}} requests={request_count}")
assert elapsed < 5.0, elapsed
"#,
        dir = dir.display().to_string(),
        base_url = base_url,
        request_count = request_count,
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python h2c perf guard");
    let server_result = server.join();
    assert!(
        output.status.success(),
        "generated Python h2c perf guard failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{}", String::from_utf8_lossy(&output.stdout).trim());
    server_result.expect("h2c perf guard server panicked");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generated_python_h2c_runtime_multiplexes_concurrent_streams() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let (base_url, server) = spawn_h2c_multiplex_server();
    let script = format!(
        r#"
import sys
from concurrent.futures import ThreadPoolExecutor
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import H2CClient
client = H2CClient(timeout=5)
def fetch(path):
    return client.get({base_url:?} + path).json()["stream"]
with ThreadPoolExecutor(max_workers=2) as pool:
    results = list(pool.map(fetch, ["/slow", "/fast"]))
client.close()
assert sorted(results) == [1, 3], results
"#,
        dir = dir.display().to_string(),
        base_url = base_url,
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python h2c multiplex smoke");
    let server_result = server.join();
    assert!(
        output.status.success(),
        "generated Python h2c multiplex smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server_result.expect("h2c multiplex server panicked");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generated_python_h2c_runtime_supports_bidi_streaming() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let (base_url, server) = spawn_h2c_bidi_server();
    let script = format!(
        r#"
import sys
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import H2CClient
client = H2CClient(timeout=5)
with client.stream("POST", {base_url:?} + "/bidi") as stream:
    stream.send_data("one\n")
    assert stream.read_chunk() == b"ack:one\n"
    stream.send_data("two\n", end_stream=True)
    rest = b"".join(stream.iter_bytes())
client.close()
assert rest == b"ack:two\n"
"#,
        dir = dir.display().to_string(),
        base_url = base_url,
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python h2c bidi smoke");
    let server_result = server.join();
    assert!(
        output.status.success(),
        "generated Python h2c bidi smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server_result.expect("h2c bidi server panicked");
    let _ = fs::remove_dir_all(&dir);
}
