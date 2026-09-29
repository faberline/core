use crate::domain::emit::ts::types_emit::HEADER;
use crate::domain::{FileBearerAuth, HttpClient};

/// Static request runtime shared by every generated client. The body depends
/// only on the chosen [`HttpClient`] backend — the `ClientConfig`/`request`
/// contract is the same, so `client.ts` and `hooks.ts` never change.
///
pub fn emit_runtime(http_client: HttpClient, auth: Option<&FileBearerAuth>) -> String {
    let body = match http_client {
        HttpClient::Fetch => FETCH_RUNTIME,
        HttpClient::Axios => AXIOS_RUNTIME,
    };
    let body = match auth {
        Some(auth) => emit_file_bearer_runtime(body, auth, http_client),
        None => body.to_string(),
    };
    format!("{HEADER}{body}")
}

fn emit_file_bearer_runtime(body: &str, auth: &FileBearerAuth, http_client: HttpClient) -> String {
    let token_path = serde_json::to_string(
        auth.token_path()
            .to_str()
            .expect("FileBearerAuth validates UTF-8 paths"),
    )
    .expect("serialize generated TypeScript token path");
    let suffix = serde_json::to_string(auth.hostname_suffix())
        .expect("serialize generated TypeScript hostname suffix");
    let schemes = auth
        .schemes()
        .map(|scheme| {
            serde_json::to_string(scheme.as_str()).expect("serialize generated TypeScript scheme")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let helper = format!(
        r##"const FILE_BEARER_TOKEN_PATH = {token_path};
const FILE_BEARER_HOSTNAME_SUFFIX = {suffix};
const FILE_BEARER_SCHEMES = new Set([{schemes}]);

function hasAuthorization(headers: Record<string, string>): boolean {{
  return Object.keys(headers).some((name) => name.toLowerCase() === "authorization");
}}

function isDnsHostname(host: string): boolean {{
  if (!host || host.length > 253 || host.endsWith(".")) return false;
  const labels = host.split(".");
  if (labels.length === 4 && labels.every((label) => /^\d{{1,3}}$/.test(label) && Number(label) <= 255)) return false;
  return labels.every((label) =>
    label.length >= 1 && label.length <= 63 &&
    /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(label)
  );
}}

function fileBearerTargetIsEligible(baseUrl: string): boolean {{
  let url: URL;
  try {{
    url = new URL(baseUrl);
  }} catch {{
    return false;
  }}
  if (!FILE_BEARER_SCHEMES.has(url.protocol.slice(0, -1))) return false;
  if (url.username || url.password || baseUrl.includes("?") || baseUrl.includes("#")) return false;
  if (url.pathname !== "" && url.pathname !== "/") return false;
  const host = url.hostname;
  if (!isDnsHostname(host) || !host.endsWith(FILE_BEARER_HOSTNAME_SUFFIX)) return false;
  const prefix = host.slice(0, -FILE_BEARER_HOSTNAME_SUFFIX.length);
  return prefix.length > 0 && !prefix.endsWith(".");
}}

async function attachFileBearer(
  baseUrl: string,
  headers: Record<string, string>,
): Promise<Record<string, string>> {{
  if (hasAuthorization(headers) || !fileBearerTargetIsEligible(baseUrl)) return headers;
  const nodeProcess = (globalThis as any).process;
  if (!nodeProcess?.versions?.node) {{
    throw new Error("request authentication token is unavailable in this runtime");
  }}
  const fsModule = "node:fs/promises";
  let raw: string;
  try {{
    const {{ readFile }} = (await import(fsModule)) as any;
    raw = await readFile(FILE_BEARER_TOKEN_PATH, "utf8");
  }} catch {{
    throw new Error("request authentication token is unavailable or invalid");
  }}
  const token = raw.trim();
  const value = `Bearer ${{token}}`;
  if (!token || /[^\x21-\x7e]/.test(token)) {{
    throw new Error("request authentication token is unavailable or invalid");
  }}
  return {{ ...headers, Authorization: value }};
}}

"##
    );
    let anchor = "/**\n * A private trust anchor is only meaningful";
    let body = body
        .replacen(anchor, &format!("{helper}{anchor}"), 1)
        // Keep the opt-in runtime executable by Node's built-in type stripper.
        // Parameter properties need a TypeScript transform, while this exact
        // equivalent only needs type removal.
        .replacen(
            "  constructor(private readonly maxInFlight: number) {}",
            "  private readonly maxInFlight: number;\n  constructor(maxInFlight: number) { this.maxInFlight = maxInFlight; }",
            1,
        );

    match http_client {
        HttpClient::Fetch => body
            .replacen(
                "  const release = await acquireAdmission(config, url);\n",
                "  const headers = await attachFileBearer(config.baseUrl, { \"Content-Type\": \"application/json\", ...config.headers, ...args.headers });\n  const release = await acquireAdmission(config, url);\n",
                1,
            )
            .replacen(
                "      headers: { \"Content-Type\": \"application/json\", ...config.headers, ...args.headers },",
                "      headers,",
                1,
            ),
        HttpClient::Axios => body
            .replacen(
                "  const release = await acquireAdmission(config, url);\n",
                "  const headers = await attachFileBearer(config.baseUrl, { \"Content-Type\": \"application/json\", ...config.headers, ...args.headers });\n  const release = await acquireAdmission(config, url);\n",
                1,
            )
            .replacen(
                "      headers: { \"Content-Type\": \"application/json\", ...config.headers, ...args.headers },",
                "      headers,",
                1,
            ),
    }
}

const FETCH_RUNTIME: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ts/fetch_runtime.ts"
));

const AXIOS_RUNTIME: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ts/axios_runtime.ts"
));
