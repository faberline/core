use super::{PdgNode, ProgramDependenceGraph, TaintAnalysis, TaintPath};
use std::collections::{HashMap, HashSet};

// ============================================================================
// Semantic taint analysis with auto-detection (R7)
// ============================================================================

/// Kind of taint source
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaintSourceKind {
    UserInput,
    EnvVar,
    HttpRequest,
    CmdArgs,
    FileRead,
    NetworkRecv,
    DeserialisedData,
    Unknown,
}

/// Kind of taint sink
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaintSinkKind {
    OsCommand,
    SubprocessExec,
    DatabaseQuery,
    CodeEval,
    FileWrite,
    NetworkSend,
    Logging,
    Unknown,
}

/// A detected taint source
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DetectedSource {
    pub line: usize,
    pub text: String,
    pub kind: TaintSourceKind,
}

/// A detected taint sink
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DetectedSink {
    pub line: usize,
    pub text: String,
    pub kind: TaintSinkKind,
}

/// Semantic taint analysis result
#[derive(Debug, Clone)]
pub struct SemanticTaintAnalysis {
    pub sources: Vec<DetectedSource>,
    pub sinks: Vec<DetectedSink>,
    pub taint_paths: Vec<TaintPath>,
}

impl SemanticTaintAnalysis {
    pub fn has_vulnerabilities(&self) -> bool {
        !self.taint_paths.is_empty()
    }
}

/// Detect taint sources by scanning statement text patterns
pub fn detect_taint_sources(nodes: &HashMap<u32, PdgNode>) -> Vec<DetectedSource> {
    let mut sources = Vec::new();

    let source_patterns: &[(&[&str], TaintSourceKind)] = &[
        (&["input(", "input ("], TaintSourceKind::UserInput),
        (
            &["os.environ", "os.getenv(", "environ.get(", "environ["],
            TaintSourceKind::EnvVar,
        ),
        (
            &[
                "request.form",
                "request.args",
                "request.json",
                "request.data",
                "request.get_json(",
                "request.values",
                "flask.request",
                "self.request",
                "HttpRequest",
            ],
            TaintSourceKind::HttpRequest,
        ),
        (
            &["sys.argv", "argparse", "click.option"],
            TaintSourceKind::CmdArgs,
        ),
        (
            &[
                "json.load(",
                "json.loads(",
                "yaml.load(",
                "pickle.load(",
                "pickle.loads(",
                "marshal.loads(",
            ],
            TaintSourceKind::DeserialisedData,
        ),
        (
            &[
                "socket.recv(",
                ".recv(",
                "socket.recvfrom(",
                "read_from_socket",
            ],
            TaintSourceKind::NetworkRecv,
        ),
        (
            &[
                "open(",
                "file.read(",
                ".read(",
                ".readline(",
                ".readlines(",
                "pathlib.Path(",
                "io.open(",
            ],
            TaintSourceKind::FileRead,
        ),
    ];

    let mut seen_lines = HashSet::new();

    for node in nodes.values() {
        if seen_lines.contains(&node.line) {
            continue;
        }

        let text_lower = node.text.to_lowercase();

        for (patterns, kind) in source_patterns {
            for pat in *patterns {
                if text_lower.contains(&pat.to_lowercase()) {
                    seen_lines.insert(node.line);
                    sources.push(DetectedSource {
                        line: node.line,
                        text: node.text.clone(),
                        kind: kind.clone(),
                    });
                    break;
                }
            }
        }
    }

    sources.sort_by_key(|s| s.line);
    sources
}

/// Detect taint sinks by scanning statement text patterns
pub fn detect_taint_sinks(nodes: &HashMap<u32, PdgNode>) -> Vec<DetectedSink> {
    let mut sinks = Vec::new();

    let sink_patterns: &[(&[&str], TaintSinkKind)] = &[
        (
            &["os.system(", "os.popen(", "os.exec", "os.spawn"],
            TaintSinkKind::OsCommand,
        ),
        (
            &[
                "subprocess.run(",
                "subprocess.call(",
                "subprocess.Popen(",
                "subprocess.check_output(",
                "subprocess.check_call(",
            ],
            TaintSinkKind::SubprocessExec,
        ),
        (
            &[
                "db.execute(",
                "cursor.execute(",
                ".execute(",
                "session.execute(",
                "connection.execute(",
                "engine.execute(",
                "raw_sql",
                "query(",
            ],
            TaintSinkKind::DatabaseQuery,
        ),
        (
            &["eval(", "exec(", "compile(", "__import__(", "importlib"],
            TaintSinkKind::CodeEval,
        ),
        (
            &[
                ".write(",
                "open(",
                "shutil.copy",
                "shutil.move",
                "pathlib.Path(",
                "json.dump(",
                "pickle.dump(",
            ],
            TaintSinkKind::FileWrite,
        ),
        (
            &[
                ".send(",
                "socket.send(",
                "requests.get(",
                "requests.post(",
                "urllib.request",
                "http.client",
                "httpx.",
            ],
            TaintSinkKind::NetworkSend,
        ),
        (
            &[
                "logging.info(",
                "logging.error(",
                "logging.warning(",
                "logging.debug(",
                "print(",
                "logger.",
            ],
            TaintSinkKind::Logging,
        ),
    ];

    let mut seen_lines = HashSet::new();

    for node in nodes.values() {
        if seen_lines.contains(&node.line) {
            continue;
        }

        let text_lower = node.text.to_lowercase();

        for (patterns, kind) in sink_patterns {
            for pat in *patterns {
                if text_lower.contains(&pat.to_lowercase()) {
                    seen_lines.insert(node.line);
                    sinks.push(DetectedSink {
                        line: node.line,
                        text: node.text.clone(),
                        kind: kind.clone(),
                    });
                    break;
                }
            }
        }
    }

    sinks.sort_by_key(|s| s.line);
    sinks
}

impl ProgramDependenceGraph {
    /// Perform semantic taint analysis with automatic source/sink detection
    ///
    /// Scans AST text for known taint source patterns (input(), os.environ,
    /// request.*, etc.) and sink patterns (os.system(), subprocess.*,
    /// db.execute(), eval(), etc.) as defined in pre-clarifications Q1.
    pub fn semantic_taint_analysis(&self) -> SemanticTaintAnalysis {
        let sources = detect_taint_sources(&self.nodes);
        let sinks = detect_taint_sinks(&self.nodes);

        let source_lines: Vec<usize> = sources.iter().map(|s| s.line).collect();
        let sink_lines: Vec<usize> = sinks.iter().map(|s| s.line).collect();

        let taint_result = self.taint_tracking(&source_lines, &sink_lines);

        SemanticTaintAnalysis {
            sources,
            sinks,
            taint_paths: taint_result.taint_paths,
        }
    }

    /// Perform taint analysis with explicit source/sink lines (R7 — manual mode)
    ///
    /// Used when the caller already knows which lines are sources and sinks.
    pub fn taint_analysis_explicit(
        &self,
        source_lines: &[usize],
        sink_lines: &[usize],
    ) -> TaintAnalysis {
        self.taint_tracking(source_lines, sink_lines)
    }
}
