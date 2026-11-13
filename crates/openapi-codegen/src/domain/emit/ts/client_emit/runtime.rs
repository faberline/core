use crate::domain::emit::ts::types_emit::HEADER;
use crate::domain::{FileBearerAuth, HttpClient, ManagedAuth};

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

/// Managed runtime: like [`emit_runtime`], but every transport entry point
/// first checks for an `https:` URL, reads the fixed token file and the CA
/// bundle, and sends the token. Other auth selections keep their own output.
pub fn emit_managed_runtime(http_client: HttpClient, auth: &ManagedAuth) -> String {
    let body = match http_client {
        HttpClient::Fetch => FETCH_RUNTIME,
        HttpClient::Axios => AXIOS_RUNTIME,
    };
    let token_path = serde_json::to_string(auth.token_path())
        .expect("serialize generated TypeScript token path");
    let ca_path = serde_json::to_string(
        auth.ca_path()
            .to_str()
            .expect("ManagedAuth validates UTF-8 paths"),
    )
    .expect("serialize generated TypeScript CA path");
    let transport = match http_client {
        HttpClient::Fetch => MANAGED_FETCH,
        HttpClient::Axios => MANAGED_AXIOS,
    };
    let helper = format!(
        "{}{}",
        MANAGED_COMMON
            .replace("__MANAGED_TOKEN_PATH__", &token_path)
            .replace("__MANAGED_CA_PATH__", &ca_path),
        transport
    );
    let anchor = "/**\n * A private trust anchor is only meaningful";
    let body = body
        .replacen(anchor, &format!("{helper}{anchor}"), 1)
        .replacen(
            "  trust?: PrivateTrust;\n",
            "  trust?: PrivateTrust;\n  /**\n   * Path to the PEM bundle of the CA that signed the server. Managed clients\n   * default to the generated `MANAGED_CA_PATH`.\n   */\n  caPath?: string;\n",
            1,
        )
        // Same type-stripper-safe constructor as the file bearer runtime.
        .replacen(
            "  constructor(private readonly maxInFlight: number) {}",
            "  private readonly maxInFlight: number;\n  constructor(maxInFlight: number) { this.maxInFlight = maxInFlight; }",
            1,
        );
    // The three lines below occur in `request` and in `sendNdjson`: replace
    // every match, so both entry points get the https check and the token.
    let body = match http_client {
        HttpClient::Fetch => body.replace(
            "  const doFetch = config.fetch ?? fetch;\n  assertTrustIsHonoured(config, config.fetch);\n  const url = new URL(config.baseUrl + args.path);\n",
            "  const url = new URL(config.baseUrl + args.path);\n  const managed = await prepareManaged(config, url);\n  const doFetch = config.fetch ?? (await managedFetch(managed.ca, url.hostname));\n  assertTrustIsHonoured(config, config.fetch);\n",
        ),
        HttpClient::Axios => body.replace(
            "  const instance = config.axios ?? axios.create();\n  assertTrustIsHonoured(config, config.axios);\n  const url = new URL(config.baseUrl + args.path);\n",
            "  const url = new URL(config.baseUrl + args.path);\n  const managed = await prepareManaged(config, url);\n  const instance = config.axios ?? (await managedAxios(managed.ca, url.hostname));\n  assertTrustIsHonoured(config, config.axios);\n",
        ),
    };
    let body = body
        .replacen(
            "      headers: { \"Content-Type\": \"application/json\", ...config.headers, ...args.headers },",
            "      headers: { \"Content-Type\": \"application/json\", ...config.headers, ...args.headers, Authorization: managed.authorization },",
            1,
        )
        .replacen(
            "  const headers = { \"Content-Type\": args.ndjsonBody ? \"application/x-ndjson\" : \"application/json\", Accept: accept, ...config.headers, ...args.headers };",
            "  const headers = { \"Content-Type\": args.ndjsonBody ? \"application/x-ndjson\" : \"application/json\", Accept: accept, ...config.headers, ...args.headers, Authorization: managed.authorization };",
            1,
        );
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
        )
        // The NDJSON helper sends its request with the same file bearer.
        .replacen(
            "  const headers = { \"Content-Type\": args.ndjsonBody ? \"application/x-ndjson\" : \"application/json\", Accept: accept, ...config.headers, ...args.headers };",
            "  const headers = await attachFileBearer(config.baseUrl, { \"Content-Type\": args.ndjsonBody ? \"application/x-ndjson\" : \"application/json\", Accept: accept, ...config.headers, ...args.headers });",
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

const MANAGED_COMMON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ts/managed_common.ts"
));

const MANAGED_FETCH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ts/managed_fetch.ts"
));

const MANAGED_AXIOS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ts/managed_axios.ts"
));
