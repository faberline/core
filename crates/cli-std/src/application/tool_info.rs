/// Identity + build provenance of the calling binary. Construct it once in the
/// binary (filling the fields from `env!`/build-script stamps) and pass it to
/// the `run` functions.
///
/// In a real binary the stamps come from `env!`/build-script values; here they
/// are literals so the example compiles standalone:
///
/// ```
/// const TOOL: cli_std::ToolInfo = cli_std::ToolInfo::new(
///     "lumen",                      // project: env-free; the tool name
///     "faberline/lumen",            // repo
///     "aarch64-apple-darwin",       // target: env!("LUMEN_TARGET") in lumen
///     env!("CARGO_PKG_VERSION"),    // version
///     "unknown",                    // git_sha: env!("LUMEN_GIT_SHA") in lumen
///     "unknown",                    // built_at: env!("LUMEN_BUILT_AT") in lumen
/// );
/// assert_eq!(TOOL.tag_prefix(), "lumen@");
/// assert_eq!(TOOL.asset_name(), "lumen-aarch64-apple-darwin.tar.gz");
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ToolInfo {
    project: &'static str,
    repo: &'static str,
    target: &'static str,
    version: &'static str,
    git_sha: &'static str,
    built_at: &'static str,
}

impl ToolInfo {
    /// The identity of a binary, from its build stamps. `const`, so a binary
    /// can keep it in a `const`.
    pub const fn new(
        project: &'static str,
        repo: &'static str,
        target: &'static str,
        version: &'static str,
        git_sha: &'static str,
        built_at: &'static str,
    ) -> Self {
        Self {
            project,
            repo,
            target,
            version,
            git_sha,
            built_at,
        }
    }

    /// Short tool name — also the release-tag prefix (`<project>@X.Y.Z`), the
    /// asset stem (`<project>-<target>.tar.gz`) and the inner binary name.
    pub const fn project(&self) -> &'static str {
        self.project
    }

    /// GitHub `owner/name` that owns releases + the issue tracker.
    pub const fn repo(&self) -> &'static str {
        self.repo
    }

    /// The exact target triple this binary was built for (build-script stamp).
    pub const fn target(&self) -> &'static str {
        self.target
    }

    /// This binary's version (`env!("CARGO_PKG_VERSION")`).
    pub const fn version(&self) -> &'static str {
        self.version
    }

    /// Short git sha stamped at build time (or "unknown").
    pub const fn git_sha(&self) -> &'static str {
        self.git_sha
    }

    /// Build timestamp stamped at build time (or "unknown").
    pub const fn built_at(&self) -> &'static str {
        self.built_at
    }

    /// Default tracker label for this tool's issue surface.
    pub fn issue_label(&self) -> String {
        format!("app:{}", self.project)
    }

    /// Release-tag prefix, e.g. `lumen@`.
    pub fn tag_prefix(&self) -> String {
        format!("{}@", self.project)
    }

    /// Release-asset filename, e.g. `lumen-aarch64-apple-darwin.tar.gz`.
    pub fn asset_name(&self) -> String {
        format!("{}-{}.tar.gz", self.project, self.target)
    }

    /// Path of the binary inside the release tarball, e.g.
    /// `lumen-aarch64-apple-darwin/lumen`.
    pub fn inner_binary_path(&self) -> String {
        format!("{}-{}/{}", self.project, self.target, self.project)
    }
}
