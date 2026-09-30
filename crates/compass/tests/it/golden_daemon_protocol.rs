//! Golden bytes for the daemon JSON-RPC protocol (compass E4).
//!
//! The daemon and `DaemonClient` exchange these messages over a socket, one
//! JSON object per line. These literals pin today's bytes so that moving the
//! protocol types between layers cannot change the wire format.

use std::fmt::Debug;

use compass::server::protocol::{
    CheckResult, DiagnosticInfo, DiagnosticsParams, ImpactNode, ImpactResult, ReferencesParams,
    Request, RequestId, Response, RpcError, SymbolInfo, TaintPathInfo, TaintResult,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Encodes `value` to `expected` and decodes `expected` back to `value`.
#[track_caller]
fn pin<T: Serialize + DeserializeOwned + Debug>(value: &T, expected: &str) {
    assert_eq!(serde_json::to_string(value).unwrap(), expected);
    let decoded: T = serde_json::from_str(expected).unwrap();
    assert_eq!(format!("{decoded:?}"), format!("{value:?}"));
}

#[test]
fn request_json_bytes_are_pinned() {
    pin(
        &Request::new(7, "check", Some(serde_json::json!({"path": "src/app.py"}))),
        "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"check\",\"params\":{\"path\":\"src/app.py\"}}",
    );
    pin(
        &Request::new("abc", "status", None),
        "{\"jsonrpc\":\"2.0\",\"id\":\"abc\",\"method\":\"status\"}",
    );
}

#[test]
fn request_without_params_field_decodes() {
    let decoded: Request =
        serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#).unwrap();
    assert_eq!(decoded.id, RequestId::Number(1));
    assert!(decoded.params.is_none());
}

#[test]
fn response_json_bytes_are_pinned() {
    pin(
        &Response::success(RequestId::Number(7), serde_json::json!({"ok": true})),
        "{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"ok\":true}}",
    );
    pin(
        &Response::error(
            RequestId::String("abc".to_string()),
            RpcError::method_not_found("nope"),
        ),
        "{\"jsonrpc\":\"2.0\",\"id\":\"abc\",\"error\":{\"code\":-32601,\"message\":\"Method not found: nope\"}}",
    );
    let with_data = RpcError {
        code: -32602,
        message: "bad params".to_string(),
        data: Some(serde_json::json!(["line"])),
    };
    pin(&Response::error(RequestId::Number(-1), with_data), "{\"jsonrpc\":\"2.0\",\"id\":-1,\"error\":{\"code\":-32602,\"message\":\"bad params\",\"data\":[\"line\"]}}");
}

#[test]
fn rpc_error_codes_are_pinned() {
    let codes = [
        RpcError::parse_error("p").code,
        RpcError::invalid_request("r").code,
        RpcError::method_not_found("m").code,
        RpcError::invalid_params("a").code,
        RpcError::internal_error("i").code,
    ];
    assert_eq!(codes, [-32700, -32600, -32601, -32602, -32603]);
}

#[test]
fn method_payload_json_bytes_are_pinned() {
    pin(
        &CheckResult {
            diagnostics: vec![DiagnosticInfo {
                file: "src/app.py".to_string(),
                line: 3,
                column: 5,
                end_line: 3,
                end_column: 11,
                severity: "warning".to_string(),
                code: "PY001".to_string(),
                message: "unused import".to_string(),
            }],
            files_checked: 2,
            errors: 0,
            warnings: 1,
        },
        "{\"diagnostics\":[{\"file\":\"src/app.py\",\"line\":3,\"column\":5,\"end_line\":3,\"end_column\":11,\"severity\":\"warning\",\"code\":\"PY001\",\"message\":\"unused import\"}],\"files_checked\":2,\"errors\":0,\"warnings\":1}",
    );
    pin(
        &SymbolInfo {
            name: "greet".to_string(),
            kind: "function".to_string(),
            line: 1,
            column: 4,
            type_info: None,
        },
        "{\"name\":\"greet\",\"kind\":\"function\",\"line\":1,\"column\":4}",
    );
    pin(&DiagnosticsParams { file: None }, "{}");
    pin(
        &ReferencesParams {
            file: "a.py".to_string(),
            line: 1,
            column: 2,
            include_declaration: true,
        },
        "{\"file\":\"a.py\",\"line\":1,\"column\":2,\"include_declaration\":true}",
    );
    pin(
        &ImpactResult {
            changed_lines: vec![2],
            affected_lines: vec![4],
            impact_tree: vec![ImpactNode {
                line: 4,
                text: "y = x".to_string(),
                reason: "data".to_string(),
                variable: Some("x".to_string()),
                children: Vec::new(),
            }],
            total_affected: 1,
        },
        "{\"changed_lines\":[2],\"affected_lines\":[4],\"impact_tree\":[{\"line\":4,\"text\":\"y = x\",\"reason\":\"data\",\"variable\":\"x\",\"children\":[]}],\"total_affected\":1}",
    );
    pin(
        &TaintResult {
            source_lines: vec![1],
            sink_lines: vec![3],
            taint_paths: vec![TaintPathInfo {
                source_line: 1,
                source_text: "q = input()".to_string(),
                source_kind: "user_input".to_string(),
                sink_line: 3,
                sink_text: "run(q)".to_string(),
                sink_kind: "sql".to_string(),
                path: vec![1, 3],
            }],
            has_vulnerabilities: true,
            auto_detected: false,
        },
        "{\"source_lines\":[1],\"sink_lines\":[3],\"taint_paths\":[{\"source_line\":1,\"source_text\":\"q = input()\",\"source_kind\":\"user_input\",\"sink_line\":3,\"sink_text\":\"run(q)\",\"sink_kind\":\"sql\",\"path\":[1,3]}],\"has_vulnerabilities\":true,\"auto_detected\":false}",
    );
}
