use super::*;

#[test]
fn generated_python_h2c_client_smoke_talks_to_real_h2c_frames() {
    let Ok(status) = Command::new("python3")
        .arg("-c")
        .arg("import pydantic")
        .status()
    else {
        return;
    };
    if !status.success() {
        return;
    }

    let out = generate(SPEC, &opts()).unwrap();
    let dir = unique_temp_dir();
    let pkg = dir.join("generated_api");
    fs::create_dir_all(&pkg).unwrap();
    for generated in &out.files {
        fs::write(pkg.join(&generated.rel_path), &generated.contents).unwrap();
    }

    let (base_url, server) = spawn_h2c_smoke_server();
    let script = format!(
        r#"
import sys
sys.path.insert(0, {dir:?})
from generated_api import Client
pet = Client({base_url:?}).get_pet_by_id(pet_id=42)
assert pet.id == 42
assert pet.name == "Ada"
assert pet.tag == "h2c"
"#,
        dir = dir.display().to_string(),
        base_url = base_url,
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python h2c smoke");
    let server_result = server.join();
    assert!(
        output.status.success(),
        "generated Python h2c smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server_result.expect("h2c smoke server panicked");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generated_python_h2c_runtime_admission_queue_times_out() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let script = format!(
        r#"
import asyncio
import sys
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import AsyncH2CClient, H2CClient, H2CTimeout

key = ("http", "example.com", 80)
client = H2CClient(max_in_flight_per_origin=1, pool_timeout=0.01)
release = client._acquire_slot(key)
try:
    try:
        client._acquire_slot(key)
        raise AssertionError("sync admission did not time out")
    except H2CTimeout:
        pass
finally:
    release()
client._acquire_slot(key)()

async def main():
    client = AsyncH2CClient(max_in_flight_per_origin=1, pool_timeout=0.01)
    release = await client._acquire_slot(key)
    try:
        try:
            await client._acquire_slot(key)
            raise AssertionError("async admission did not time out")
        except H2CTimeout:
            pass
    finally:
        release()
    release_again = await client._acquire_slot(key)
    release_again()

asyncio.run(main())
"#,
        dir = dir.display().to_string(),
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python h2c admission smoke");
    assert!(
        output.status.success(),
        "generated Python h2c admission smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generated_python_async_h2c_runtime_smoke_talks_to_real_h2c_frames() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let (base_url, server) = spawn_h2c_smoke_server();
    let script = format!(
        r#"
import asyncio
import sys
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import AsyncH2CClient
async def main():
    async with AsyncH2CClient(timeout=5) as client:
        response = await client.get({base_url:?} + "/pets/42")
        payload = response.json()
        assert payload["id"] == 42
        assert payload["name"] == "Ada"
        assert payload["tag"] == "h2c"
asyncio.run(main())
"#,
        dir = dir.display().to_string(),
        base_url = base_url,
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python async h2c smoke");
    let server_result = server.join();
    assert!(
        output.status.success(),
        "generated Python async h2c smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server_result.expect("h2c async smoke server panicked");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generated_python_async_client_smoke_validates_pydantic_response() {
    let Ok(status) = Command::new("python3")
        .arg("-c")
        .arg("import pydantic")
        .status()
    else {
        return;
    };
    if !status.success() {
        return;
    }

    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let (base_url, server) = spawn_h2c_smoke_server();
    let script = format!(
        r#"
import asyncio
import sys
sys.path.insert(0, {dir:?})
from generated_api import AsyncClient
async def main():
    async with AsyncClient({base_url:?}) as client:
        pet = await client.get_pet_by_id(pet_id=42)
        assert pet.id == 42
        assert pet.name == "Ada"
        assert pet.tag == "h2c"
asyncio.run(main())
"#,
        dir = dir.display().to_string(),
        base_url = base_url,
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python async client smoke");
    let server_result = server.join();
    assert!(
        output.status.success(),
        "generated Python async client smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server_result.expect("h2c async client smoke server panicked");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generated_python_async_h2c_runtime_supports_bidi_streaming() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = write_generated_python_package(&out);
    let (base_url, server) = spawn_h2c_bidi_server();
    let script = format!(
        r#"
import asyncio
import sys
sys.path.insert(0, {dir:?} + "/generated_api")
from h2c_runtime import AsyncH2CClient
async def main():
    async with AsyncH2CClient(timeout=5) as client:
        async with client.stream("POST", {base_url:?} + "/bidi") as stream:
            await stream.send_data("one\n")
            assert await stream.read_chunk() == b"ack:one\n"
            await stream.send_data("two\n", end_stream=True)
            chunks = []
            async for chunk in stream.iter_bytes():
                chunks.append(chunk)
    assert b"".join(chunks) == b"ack:two\n"
asyncio.run(main())
"#,
        dir = dir.display().to_string(),
        base_url = base_url,
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python async h2c bidi smoke");
    let server_result = server.join();
    assert!(
        output.status.success(),
        "generated Python async h2c bidi smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server_result.expect("h2c async bidi server panicked");
    let _ = fs::remove_dir_all(&dir);
}
