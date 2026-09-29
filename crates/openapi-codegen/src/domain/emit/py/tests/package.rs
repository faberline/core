use super::*;

#[test]
fn emits_models_client_init() {
    let out = generate(SPEC, &opts()).unwrap();
    let names: Vec<&str> = out.files.iter().map(|f| f.rel_path.as_str()).collect();
    assert_eq!(
        names,
        vec!["models.py", "h2c_runtime.py", "client.py", "__init__.py"]
    );
}

#[test]
fn pydantic_model_has_typed_required_and_optional_fields() {
    let out = generate(SPEC, &opts()).unwrap();
    let models = file(&out, "models.py");
    assert!(models.contains("from pydantic import BaseModel, Field"));
    assert!(models.contains("class Pet(BaseModel):"));
    assert!(models.contains("    id: int\n"));
    assert!(models.contains("    name: str\n"));
    assert!(models.contains("    tag: Optional[str] = None\n"));
}

#[test]
fn pydantic_models_preserve_inline_oneof_object_variants() {
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

    let out = generate(RECURSIVE_UNION_SPEC, &opts()).unwrap();
    let models = file(&out, "models.py");
    assert!(models.contains("from typing import Any, Literal, Optional, Union"));
    assert!(models.contains("from pydantic import BaseModel, Field, RootModel"));
    assert!(models.contains("class QueryNodeMatch(BaseModel):"));
    assert!(models.contains("class QueryNodeTerm(BaseModel):"));
    assert!(models.contains("class QueryNodeAnd(BaseModel):"));
    assert!(models.contains("    and_: list[QueryNode] = Field(alias=\"and\")"));
    assert!(models.contains("class QueryNodeNot(BaseModel):"));
    assert!(models.contains("    not_: QueryNode = Field(alias=\"not\")"));
    assert!(models.contains(
        "class QueryNode(RootModel[Union[QueryNodeMatch, QueryNodeTerm, QueryNodeAnd, QueryNodeNot]]):"
    ));

    let dir = write_generated_python_package(&out);
    let script = format!(
        r#"
import sys
sys.path.insert(0, {dir:?})
from generated_api import QueryNode, QueryNodeAnd, QueryNodeMatch, SearchRequest

node = QueryNode.model_validate({{"and": [
    {{"match": {{"field": "body", "text": "hello"}}}},
    {{"not": {{"term": {{"field": "status", "value": "draft"}}}}}},
]}})
assert isinstance(node.root, QueryNodeAnd), type(node.root)
assert isinstance(node.root.and_[0].root, QueryNodeMatch), type(node.root.and_[0].root)
search = SearchRequest.model_validate({{"query": {{"match": {{"field": "body", "text": "hello"}}}}}})
assert isinstance(search.query.root, QueryNodeMatch), type(search.query.root)
QueryNode.model_json_schema()
"#,
        dir = dir.display().to_string(),
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python oneOf pydantic smoke");
    assert!(
        output.status.success(),
        "generated Python oneOf pydantic smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn h2c_default_client_method_is_typed_and_validated() {
    let out = generate(SPEC, &opts()).unwrap();
    let client = file(&out, "client.py");
    assert!(client.contains("class SupportsRequest(Protocol):"));
    assert!(client.contains("from .h2c_runtime import AsyncH2CClient, H2CClient"));
    assert!(client.contains("class Client:"));
    assert!(client.contains("client: Optional[SupportsRequest] = None"));
    assert!(client.contains("default_headers: Optional[Mapping[str, Any]] = None"));
    assert!(client.contains("auth_token: Optional[str] = None"));
    assert!(client.contains("self._client = client or H2CClient(ssl_context=ssl_context)"));
    assert!(client.contains("self._default_headers: dict[str, Any] = dict(default_headers or {})"));
    assert!(client.contains("self._default_headers[\"Authorization\"] = f\"Bearer {auth_token}\""));
    assert!(client.contains("def __enter__(self) -> \"Client\":"));
    assert!(client.contains("def close(self) -> None:"));
    assert!(client.contains("def get_pet_by_id(self, *, pet_id: int) -> Pet:"));
    assert!(client.contains("_path = f\"/pets/{pet_id}\""));
    assert!(client.contains("_headers: dict[str, Any] = dict(self._default_headers)"));
    assert!(client.contains("self._client.request(\"GET\""));
    assert!(client.contains("return Pet.model_validate(_resp.json())"));
    assert!(client.contains("class AsyncSupportsRequest(Protocol):"));
    assert!(client.contains("class AsyncClient:"));
    assert!(client.contains("self._client = client or AsyncH2CClient(ssl_context=ssl_context)"));
    assert!(client.contains("async def __aenter__(self) -> \"AsyncClient\":"));
    assert!(client.contains("async def aclose(self) -> None:"));
    assert!(client.contains("async def get_pet_by_id(self, *, pet_id: int) -> Pet:"));
    assert!(client.contains("_resp = await self._client.request(\"GET\""));

    let init = file(&out, "__init__.py");
    assert!(init.contains("from .client import AsyncClient, Client"));
    assert!(init.contains("AsyncH2CClient, AsyncH2CConnection, AsyncH2CStream"));
    assert!(init.contains("H2CClient, H2CConnection, H2CResponse, H2CStream"));
}

#[test]
fn generated_python_client_merges_auth_defaults_into_method_headers() {
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
    let script = format!(
        r#"
import asyncio
import sys
sys.path.insert(0, {dir:?})
from generated_api.client import AsyncClient, Client

class Response:
    def raise_for_status(self):
        pass
    def json(self):
        return {{"id": 1, "name": "n"}}

class Fake:
    def __init__(self):
        self.calls = []
    def request(self, method, url, *, params, headers, json=None, data=None, content=None, timeout=None):
        self.calls.append((method, url, dict(headers)))
        return Response()

class AsyncFake:
    def __init__(self):
        self.calls = []
    async def request(self, method, url, *, params, headers, json=None, data=None, content=None, timeout=None):
        self.calls.append((method, url, dict(headers)))
        return Response()

sync = Fake()
Client("http://example", client=sync, default_headers={{"X-Trace": "t"}}, auth_token="tok").get_pet_by_id(pet_id=1)
assert sync.calls[0][2]["Authorization"] == "Bearer tok", sync.calls
assert sync.calls[0][2]["X-Trace"] == "t", sync.calls

async def main():
    transport = AsyncFake()
    await AsyncClient("http://example", client=transport, default_headers={{"Authorization": "Bearer explicit"}}).get_pet_by_id(pet_id=2)
    assert transport.calls[0][2]["Authorization"] == "Bearer explicit", transport.calls

asyncio.run(main())
"#,
        dir = dir.display().to_string(),
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run generated Python client auth-default smoke");
    assert!(
        output.status.success(),
        "generated Python client auth-default smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn deterministic() {
    let a = generate(SPEC, &opts()).unwrap();
    let b = generate(SPEC, &opts()).unwrap();
    for (fa, fb) in a.files.iter().zip(b.files.iter()) {
        assert_eq!(fa.contents, fb.contents);
    }
}
