//! JSON-RPC protocol definitions for Argus daemon

pub use crate::domain::daemon::protocol::{
    CheckParams, CheckResult, DefinitionParams, DiagnosticInfo, DiagnosticsParams, HoverParams,
    ImpactNode, ImpactParams, ImpactResult, IndexStatus, LocationInfo, PdgParams, ReferencesParams,
    Request, RequestId, Response, RpcError, SliceNodeInfo, SliceParams, SliceResult, SymbolInfo,
    SymbolsParams, TaintParams, TaintPathInfo, TaintResult, TypeAtParams,
};
