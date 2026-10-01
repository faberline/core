use super::*;

#[test]
fn generated_python_files_compile_when_python3_available() {
    let out = generate(SPEC, &opts()).unwrap();
    let dir = unique_temp_dir();
    fs::create_dir_all(&dir).unwrap();
    for generated in &out.files {
        fs::write(dir.join(&generated.rel_path), &generated.contents).unwrap();
    }

    let status = match Command::new("python3")
        .arg("-m")
        .arg("py_compile")
        .arg(dir.join("models.py"))
        .arg(dir.join("h2c_runtime.py"))
        .arg(dir.join("client.py"))
        .arg(dir.join("__init__.py"))
        .status()
    {
        Ok(status) => status,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => panic!("failed to run python3: {err}"),
    };
    assert!(status.success(), "generated Python files failed py_compile");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generated_32_query_operation_client_compiles_when_python3_available() {
    let out = generate(SPEC_32_QUERY, &opts()).unwrap();
    let client = file(&out, "client.py");
    assert!(client.contains("def search_pets(self, *, body:"));
    assert!(client.contains("_method = \"QUERY\""));
    assert!(client.contains("_method = \"POST\""));

    let dir = unique_temp_dir();
    fs::create_dir_all(&dir).unwrap();
    for generated in &out.files {
        fs::write(dir.join(&generated.rel_path), &generated.contents).unwrap();
    }

    let status = match Command::new("python3")
        .arg("-m")
        .arg("py_compile")
        .arg(dir.join("models.py"))
        .arg(dir.join("h2c_runtime.py"))
        .arg(dir.join("client.py"))
        .arg(dir.join("__init__.py"))
        .status()
    {
        Ok(status) => status,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => panic!("failed to run python3: {err}"),
    };
    assert!(
        status.success(),
        "generated OpenAPI 3.2 QUERY-operation Python files failed py_compile"
    );
    let _ = fs::remove_dir_all(&dir);
}
