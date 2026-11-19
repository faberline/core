//! The `spec` verb: print a service's API document and run `spec gen`.
//! Clap-agnostic; generation is a callback because `openapi-codegen` depends on `cli-std`.

use serde_json::{Map, Value};
use std::io::Write;
use std::path::PathBuf;

/// Exit code: success.
pub const EXIT_OK: i32 = 0;
/// Exit code: runtime error (source or generation failed).
pub const EXIT_ERROR: i32 = 1;
/// Exit code: usage error (what clap gives for a bad flag).
pub const EXIT_USAGE: i32 = 2;

/// A document source: a service library function. Offline; reads no file or network.
pub type Source = fn() -> anyhow::Result<String>;

/// The document formats `spec` can print.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// OpenAPI as JSON.
    OpenApi,
    /// OpenAPI as YAML.
    OpenApiYaml,
    /// JSON Schema.
    JsonSchema,
    /// The route table.
    Routes,
}

/// What stdout carries after or inside the document (research diff 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextMarker {
    /// stdout is the raw document (tape, keep, relay).
    None,
    /// A plain line `next: <value>` after the document (defer: value `done`).
    Line(&'static str),
    /// sift: OpenAPI JSON is wrapped as `{"<wrap_openapi_as>": doc, "next": value}`;
    /// other JSON gets `"next"` added; YAML gets a `# next: <value>` line.
    Embedded {
        /// The key that wraps the OpenAPI document.
        wrap_openapi_as: &'static str,
        /// The value of the `next` marker.
        value: &'static str,
    },
}

/// How `spec gen` ends (research diff 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenEnd {
    /// `next: <out>/<entry file>`.
    EntryFile,
    /// `next: done`.
    Done,
}

/// A target language for `spec gen`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    /// TypeScript.
    Ts,
    /// Python.
    Py,
    /// Rust.
    Rust,
}

impl Lang {
    /// The name the CLI takes: `ts`, `py` or `rust`.
    pub const fn name(self) -> &'static str {
        match self {
            Lang::Ts => "ts",
            Lang::Py => "py",
            Lang::Rust => "rust",
        }
    }

    /// The entry file that `spec gen` writes last.
    pub const fn entry_file(self) -> &'static str {
        match self {
            Lang::Ts => "index.ts",
            Lang::Py => "__init__.py",
            Lang::Rust => "mod.rs",
        }
    }
}

/// The `spec gen` parameters that differ per service.
#[derive(Clone, Copy, Debug)]
pub struct GenParams {
    /// The factory function name, for example `createClient`.
    pub factory_name: &'static str,
    /// Whether `--target` is accepted.
    pub accepts_target: bool,
    /// Whether `--http` is accepted.
    pub accepts_http: bool,
    /// How the output ends.
    pub end: GenEnd,
}

/// Everything that differs per service.
#[derive(Clone, Debug)]
pub struct SpecParams {
    openapi_json: Source,
    openapi_yaml: Option<Source>,
    json_schema: Option<Source>,
    routes: Option<Source>,
    catalogs: Vec<(&'static str, Source)>,
    openapi_name: &'static str,
    yaml_aliases: &'static [&'static str],
    next_marker: NextMarker,
    gen: Option<GenParams>,
}

impl SpecParams {
    /// Starts from the OpenAPI JSON source; the other settings have defaults.
    pub fn new(openapi_json: Source) -> Self {
        Self {
            openapi_json,
            openapi_yaml: None,
            json_schema: None,
            routes: None,
            catalogs: Vec::new(),
            openapi_name: "openapi",
            yaml_aliases: &[],
            next_marker: NextMarker::None,
            gen: None,
        }
    }

    /// Sets the OpenAPI YAML source.
    pub fn with_openapi_yaml(mut self, source: Source) -> Self {
        self.openapi_yaml = Some(source);
        self
    }

    /// Sets the JSON Schema source.
    pub fn with_json_schema(mut self, source: Source) -> Self {
        self.json_schema = Some(source);
        self
    }

    /// Sets the route table source.
    pub fn with_routes(mut self, source: Source) -> Self {
        self.routes = Some(source);
        self
    }

    /// Adds a catalog (keep: `shapes`, `fields`). May be called more than once.
    pub fn with_catalog(mut self, name: &'static str, source: Source) -> Self {
        self.catalogs.push((name, source));
        self
    }

    /// Sets the name of the OpenAPI JSON format (sift: `openapi-json`).
    pub fn with_openapi_name(mut self, name: &'static str) -> Self {
        self.openapi_name = name;
        self
    }

    /// Sets the aliases of the YAML format (keep, relay: `yaml`, `openapi.yaml`).
    pub fn with_yaml_aliases(mut self, aliases: &'static [&'static str]) -> Self {
        self.yaml_aliases = aliases;
        self
    }

    /// Sets what stdout carries after the document.
    pub fn with_next_marker(mut self, marker: NextMarker) -> Self {
        self.next_marker = marker;
        self
    }

    /// Enables `spec gen` with these parameters.
    pub fn with_gen(mut self, gen: GenParams) -> Self {
        self.gen = Some(gen);
        self
    }

    /// The name of the OpenAPI JSON format.
    pub fn default_format_name(&self) -> &'static str {
        self.openapi_name
    }

    /// Parses a `--format` value. A format whose source is not set is an error.
    pub fn parse_format(&self, value: &str) -> Result<Format, String> {
        if value == self.openapi_name {
            return Ok(Format::OpenApi);
        }
        if self.openapi_yaml.is_some()
            && (value == "openapi-yaml" || self.yaml_aliases.contains(&value))
        {
            return Ok(Format::OpenApiYaml);
        }
        if self.json_schema.is_some() && value == "json-schema" {
            return Ok(Format::JsonSchema);
        }
        if self.routes.is_some() && value == "routes" {
            return Ok(Format::Routes);
        }
        Err(format!("unsupported format `{value}`"))
    }

    fn source_for(&self, format: Format) -> Option<Source> {
        match format {
            Format::OpenApi => Some(self.openapi_json),
            Format::OpenApiYaml => self.openapi_yaml,
            Format::JsonSchema => self.json_schema,
            Format::Routes => self.routes,
        }
    }

    fn format_name(&self, format: Format) -> &'static str {
        match format {
            Format::OpenApi => self.openapi_name,
            Format::OpenApiYaml => "openapi-yaml",
            Format::JsonSchema => "json-schema",
            Format::Routes => "routes",
        }
    }
}

/// Flags for a print run.
#[derive(Clone, Debug)]
pub struct PrintOptions {
    format: Format,
    catalog: Option<String>,
}

impl PrintOptions {
    /// Prints this format.
    pub fn new(format: Format) -> Self {
        Self {
            format,
            catalog: None,
        }
    }

    /// Prints this catalog instead of the format (the caller picks the first flag set).
    pub fn with_catalog(mut self, name: impl Into<String>) -> Self {
        self.catalog = Some(name.into());
        self
    }
}

/// The inputs of a `spec gen` run.
#[derive(Clone, Debug)]
pub struct GenRequest {
    /// The target language.
    pub lang: Lang,
    /// The output directory.
    pub out: PathBuf,
    /// The `--target` value, if given.
    pub target: Option<String>,
    /// The `--http` value, if given.
    pub http: Option<String>,
    /// The factory name. `run_gen` replaces it with the one from the params.
    pub factory_name: &'static str,
}

/// What the generation callback returns.
#[derive(Clone, Debug, Default)]
pub struct GenOutcome {
    /// One `generated <path>` line each.
    pub files: Vec<PathBuf>,
    /// One more `generated <path>` line (tape, keep, relay: `.openapi-codegen.json`).
    pub manifest: Option<PathBuf>,
    /// `(target, minimum version)`, printed as `target: <t> (minimum <lang> <v>)`.
    pub target: Option<(String, String)>,
}

fn fail(err: &mut dyn Write, code: i32, message: impl std::fmt::Display) -> i32 {
    let _ = writeln!(err, "Error: {message}");
    code
}

/// Prints the document for `opts`. The return value is the exit code.
pub fn run_print(
    params: &SpecParams,
    opts: &PrintOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let source = match &opts.catalog {
        Some(name) => match params.catalogs.iter().find(|(n, _)| *n == name.as_str()) {
            Some((_, source)) => *source,
            None => {
                return fail(
                    err,
                    EXIT_USAGE,
                    format_args!("unsupported catalog `{name}`"),
                )
            }
        },
        None => match params.source_for(opts.format) {
            Some(source) => source,
            None => {
                let name = params.format_name(opts.format);
                return fail(err, EXIT_USAGE, format_args!("unsupported format `{name}`"));
            }
        },
    };
    let doc = match source() {
        Ok(doc) => doc,
        Err(e) => return fail(err, EXIT_ERROR, e),
    };
    let doc = doc.trim_end_matches('\n');
    if let NextMarker::Embedded {
        wrap_openapi_as,
        value,
    } = params.next_marker
    {
        return match embed(doc, opts, wrap_openapi_as, value) {
            Ok(text) => {
                let _ = writeln!(out, "{text}");
                EXIT_OK
            }
            Err(e) => fail(err, EXIT_ERROR, e),
        };
    }
    let _ = writeln!(out, "{doc}");
    if let NextMarker::Line(value) = params.next_marker {
        let _ = writeln!(out, "next: {value}");
    }
    EXIT_OK
}

fn embed(
    doc: &str,
    opts: &PrintOptions,
    wrap_openapi_as: &str,
    value: &str,
) -> anyhow::Result<String> {
    let json_like =
        opts.catalog.is_some() || matches!(opts.format, Format::JsonSchema | Format::Routes);
    let mut object = if json_like {
        match serde_json::from_str::<Value>(doc)? {
            Value::Object(object) => object,
            _ => anyhow::bail!("document is not a JSON object"),
        }
    } else if opts.format == Format::OpenApi {
        let mut wrapped = Map::new();
        wrapped.insert(wrap_openapi_as.to_string(), serde_json::from_str(doc)?);
        wrapped
    } else {
        return Ok(format!("{doc}\n# next: {value}"));
    };
    object.insert("next".to_string(), Value::String(value.to_string()));
    Ok(serde_json::to_string_pretty(&Value::Object(object))?)
}

/// Runs `spec gen`: checks flags, reads the OpenAPI JSON, calls `generate`, prints the outcome.
pub fn run_gen(
    params: &SpecParams,
    req: &GenRequest,
    generate: &dyn Fn(&GenRequest, &str) -> anyhow::Result<GenOutcome>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let Some(gen) = params.gen else {
        return fail(err, EXIT_USAGE, "spec gen is not configured");
    };
    if req.target.is_some() && !gen.accepts_target {
        return fail(err, EXIT_USAGE, "unsupported option `--target`");
    }
    if req.http.is_some() && !gen.accepts_http {
        return fail(err, EXIT_USAGE, "unsupported option `--http`");
    }
    let doc = match (params.openapi_json)() {
        Ok(doc) => doc,
        Err(e) => return fail(err, EXIT_ERROR, e),
    };
    let mut request = req.clone();
    request.factory_name = gen.factory_name;
    let outcome = match generate(&request, &doc) {
        Ok(outcome) => outcome,
        Err(e) => return fail(err, EXIT_ERROR, e),
    };
    for file in &outcome.files {
        let _ = writeln!(out, "generated {}", file.display());
    }
    if let Some(manifest) = &outcome.manifest {
        let _ = writeln!(out, "generated {}", manifest.display());
    }
    if let Some((target, minimum)) = &outcome.target {
        let lang = req.lang.name();
        let _ = writeln!(out, "target: {target} (minimum {lang} {minimum})");
    }
    match gen.end {
        GenEnd::EntryFile => {
            let _ = writeln!(out, "next: {}/{}", req.out.display(), req.lang.entry_file());
        }
        GenEnd::Done => {
            let _ = writeln!(out, "next: done");
        }
    }
    EXIT_OK
}
#[cfg(test)]
mod tests;
