//! The `dockerfile` verb: render a service Dockerfile (`source` or `release`)
//! from parameters. Clap-agnostic: the caller passes the output writers and
//! gets an exit code back.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::application::spec::{EXIT_ERROR, EXIT_OK};
use crate::interfaces::artifact::{
    ensure_trailing_newline, release_tag, strip_source_ownership_markers,
};

/// Default file name of the `source` render.
pub const DEFAULT_SOURCE_FILE: &str = "Dockerfile";
/// Default file name of the `release` render.
pub const DEFAULT_RELEASE_FILE: &str = "Dockerfile.release";

/// Which embedded text the render uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// The source Dockerfile, for building from the repository.
    Source,
    /// The release Dockerfile, with the version filled in.
    Release,
}

/// How the version appears in the output (research diff 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionForm {
    /// `<project>@<v>`.
    Tagged,
    /// `<v>`.
    Bare,
}

/// What the release render rewrites (research diff 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseRewrite {
    /// The `docker build` comment lines and the `ARG` line (tape, keep, relay).
    CommentsAndArg,
    /// Only the `ARG` line (defer).
    ArgOnly,
    /// Replace the `<ARG>=REPLACE_ME` placeholder on every line (sift).
    Placeholder,
}

/// Which lines the render drops (research diff 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerRule {
    /// Drop the source-ownership and codegen marker lines.
    SourceOwnership,
    /// Drop the `HANDWRITE-BEGIN` and `HANDWRITE-END` marker lines.
    Handwrite,
}

/// How `--out` picks the file (research diff 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutPathRule {
    /// A name with an extension is the file; a name without one is a directory.
    ByExtension,
    /// An existing directory gets the default file name; any other name is the file.
    ByFileSystem,
}

/// What stdout carries after the text (research diff 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// Nothing after the text (tape, keep).
    None,
    /// `next: done` only after a file write (relay).
    DoneOnFile,
    /// `next: done` after the stdout text and after a file write (defer).
    DoneAlways,
    /// `next: <stdout>` after the text; `next: <file_prefix><path>` after a file write (sift).
    Lines {
        /// The line printed after the text on stdout.
        stdout: &'static str,
        /// The prefix of the line printed after a file write.
        file_prefix: &'static str,
    },
}

/// The per-service parameters of the `dockerfile` verb.
#[derive(Clone, Debug)]
pub struct DockerfileParams {
    project: &'static str,
    build_arg: &'static str,
    source: &'static str,
    release: &'static str,
    fallback_version: &'static str,
    default_variant: Variant,
    version_form: VersionForm,
    release_rewrite: ReleaseRewrite,
    marker_rule: MarkerRule,
    out_path_rule: OutPathRule,
    next: Next,
}

impl DockerfileParams {
    /// `project` (tag prefix and image name), `build_arg` (for example
    /// `TAPE_VERSION`), the two embedded texts, and the fallback version (the
    /// package version).
    pub fn new(
        project: &'static str,
        build_arg: &'static str,
        source: &'static str,
        release: &'static str,
        fallback_version: &'static str,
    ) -> Self {
        Self {
            project,
            build_arg,
            source,
            release,
            fallback_version,
            default_variant: Variant::Source,
            version_form: VersionForm::Tagged,
            release_rewrite: ReleaseRewrite::CommentsAndArg,
            marker_rule: MarkerRule::SourceOwnership,
            out_path_rule: OutPathRule::ByExtension,
            next: Next::None,
        }
    }

    /// Set the variant used when the request does not name one.
    pub fn with_default_variant(mut self, v: Variant) -> Self {
        self.default_variant = v;
        self
    }

    /// Set how the version appears in the output.
    pub fn with_version_form(mut self, f: VersionForm) -> Self {
        self.version_form = f;
        self
    }

    /// Set what the release render rewrites.
    pub fn with_release_rewrite(mut self, r: ReleaseRewrite) -> Self {
        self.release_rewrite = r;
        self
    }

    /// Set which marker lines the render drops.
    pub fn with_marker_rule(mut self, m: MarkerRule) -> Self {
        self.marker_rule = m;
        self
    }

    /// Set how `--out` picks the file.
    pub fn with_out_path_rule(mut self, r: OutPathRule) -> Self {
        self.out_path_rule = r;
        self
    }

    /// Set what stdout carries after the text.
    pub fn with_next(mut self, n: Next) -> Self {
        self.next = n;
        self
    }

    /// The variant the CLI uses as its clap default.
    pub fn default_variant(&self) -> Variant {
        self.default_variant
    }
}

/// The request of one `dockerfile` render, from the command line.
#[derive(Clone, Debug, Default)]
pub struct DockerfileRequest {
    /// `None` uses the parameters' default variant.
    pub variant: Option<Variant>,
    /// `--version`.
    pub version: Option<String>,
    /// `--out`.
    pub out: Option<PathBuf>,
}

/// Render the Dockerfile of the request. Writes the text to `out` (or the file
/// named by `--out`) and the `next:` line; errors go to `err`. Returns the exit code.
pub fn run_render(
    params: &DockerfileParams,
    req: &DockerfileRequest,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let variant = req.variant.unwrap_or(params.default_variant);
    let base = match variant {
        Variant::Source => params.source,
        Variant::Release => params.release,
    };
    let stripped = drop_markers(base, params.marker_rule);
    let text = match variant {
        Variant::Source => stripped,
        Variant::Release => match render_release(params, req, &stripped) {
            Ok(text) => text,
            Err(message) => {
                let _ = writeln!(err, "{message}");
                return EXIT_ERROR;
            }
        },
    };
    match emit(params, req, variant, &text, out) {
        Ok(()) => EXIT_OK,
        Err(e) => {
            let _ = writeln!(err, "Error: {e}");
            EXIT_ERROR
        }
    }
}

fn drop_markers(text: &str, rule: MarkerRule) -> String {
    match rule {
        MarkerRule::SourceOwnership => strip_source_ownership_markers(text),
        MarkerRule::Handwrite => text
            .lines()
            .filter(|line| !line.contains("HANDWRITE-BEGIN") && !line.contains("HANDWRITE-END"))
            .map(|line| format!("{line}\n"))
            .collect(),
    }
}

fn render_release(
    params: &DockerfileParams,
    req: &DockerfileRequest,
    stripped: &str,
) -> Result<String, &'static str> {
    let project_prefix = format!("{}@", params.project);
    let (tag, bare) = match params.version_form {
        VersionForm::Tagged => {
            let tag = release_tag(
                params.project,
                req.version.as_deref(),
                params.fallback_version,
            );
            let bare = tag
                .strip_prefix(&project_prefix)
                .unwrap_or(&tag)
                .to_string();
            (tag, bare)
        }
        VersionForm::Bare => {
            if let Some(v) = req.version.as_deref() {
                if v.trim().is_empty() {
                    return Err("Error: release Dockerfile version must not be empty");
                }
            }
            let raw = req
                .version
                .as_deref()
                .unwrap_or(params.fallback_version)
                .trim();
            let bare = raw.strip_prefix(&project_prefix).unwrap_or(raw).to_string();
            (bare.clone(), bare)
        }
    };
    let joined = stripped
        .lines()
        .map(|line| rewrite_line(params, line, &tag, &bare))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(ensure_trailing_newline(&joined))
}

fn rewrite_line(params: &DockerfileParams, line: &str, tag: &str, bare: &str) -> String {
    let build_arg = params.build_arg;
    let arg_prefix = format!("ARG {build_arg}=");
    match params.release_rewrite {
        ReleaseRewrite::CommentsAndArg => {
            let docker_prefix = format!(
                "#   docker build -f Dockerfile.release -t {}:",
                params.project
            );
            let build_arg_prefix = format!("#     --build-arg {build_arg}=");
            if line.starts_with(&docker_prefix) {
                format!(
                    "#   docker build -f Dockerfile.release -t {}:{bare} \\",
                    params.project
                )
            } else if line.starts_with(&build_arg_prefix) {
                format!("#     --build-arg {build_arg}={tag} .")
            } else if line.starts_with(&arg_prefix) {
                format!("ARG {build_arg}={tag}")
            } else {
                line.to_string()
            }
        }
        ReleaseRewrite::ArgOnly => {
            if line.starts_with(&arg_prefix) {
                format!("ARG {build_arg}={tag}")
            } else {
                line.to_string()
            }
        }
        ReleaseRewrite::Placeholder => line.replace(
            &format!("{build_arg}=REPLACE_ME"),
            &format!("{build_arg}={bare}"),
        ),
    }
}

fn emit(
    params: &DockerfileParams,
    req: &DockerfileRequest,
    variant: Variant,
    text: &str,
    out: &mut dyn Write,
) -> io::Result<()> {
    let default_file = match variant {
        Variant::Source => DEFAULT_SOURCE_FILE,
        Variant::Release => DEFAULT_RELEASE_FILE,
    };
    match req.out.as_deref() {
        None => {
            write!(out, "{text}")?;
            write_next(params.next, None, out)
        }
        Some(path) => {
            let target = target_path(path, default_file, params.out_path_rule);
            if let Some(parent) = target.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(&target, text)?;
            writeln!(out, "wrote {}", target.display())?;
            write_next(params.next, Some(&target), out)
        }
    }
}

fn target_path(path: &Path, default_file: &str, rule: OutPathRule) -> PathBuf {
    let is_directory = match rule {
        OutPathRule::ByExtension => path.extension().is_none(),
        OutPathRule::ByFileSystem => path.is_dir(),
    };
    if is_directory {
        path.join(default_file)
    } else {
        path.to_path_buf()
    }
}

fn write_next(next: Next, written: Option<&Path>, out: &mut dyn Write) -> io::Result<()> {
    match (next, written) {
        (Next::None, _) => Ok(()),
        (Next::DoneOnFile, None) => Ok(()),
        (Next::DoneOnFile, Some(_)) | (Next::DoneAlways, _) => writeln!(out, "next: done"),
        (Next::Lines { stdout, .. }, None) => writeln!(out, "next: {stdout}"),
        (Next::Lines { file_prefix, .. }, Some(path)) => {
            writeln!(out, "next: {file_prefix}{}", path.display())
        }
    }
}

#[cfg(test)]
mod tests;
