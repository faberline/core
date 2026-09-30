# service-mcp

service-mcp is the shared transport shell for a Model Context Protocol server:
it serves an MCP application over stdio or over streamable HTTP, with a browser
security policy of allowed hosts and allowed origins. The protocol itself is
rmcp's. Tool schemas, tool handlers and the decision of what a bearer token may
do stay in the product, which supplies them as an `McpApplication`. No core
crate depends on it; downstream, sift serves its MCP server through it.

**Form:** whole-src interfaces · **Depends on:** service-auth · **Crate:** [`crates/service-mcp`](../../crates/service-mcp)

## Model

- **MCP application** — `McpApplication`: an rmcp `ServerHandler` that can be
  cloned per request and bound to the caller's bearer token through
  `with_bearer_token`.
- **HTTP transport config** — `HttpTransportConfig`: the allowed hosts and the
  allowed origins of the streamable HTTP transport. `from_env` reads each list
  as comma-separated values from a named variable, with a default when the
  variable is unset.
- **stdio transport** — `serve_stdio`: serves one application over stdin and
  stdout until the peer ends the session.
- **Streamable HTTP route** — `streamable_http_router`: an axum `Router` with
  one route at the given path, whose sessions live in one in-process session
  manager shared by every request to that router.

## Ports

- `McpApplication` — implemented by the product; sift implements it.

## Invariants

- `HttpTransportConfig` holds at least one allowed host and at least one
  allowed origin, and no blank value. A variable that is set but yields no
  value is an error, not a fallback to the default.
- Hosts and origins are enforced by rmcp's streamable HTTP server: a request
  from an origin that is not allowed is refused with 403.
- Each HTTP request takes its bearer token with service-auth's `bearer_token`
  and hands it, or `None`, to the application through `with_bearer_token`.
  This crate never verifies the token; the application decides what it allows.
- An accepted initialize request gets an `mcp-session-id` header.

## Published language

The whole public API; whole-src interfaces contexts have no application layer.
A sift test reads sift's own sources and asserts the paths
`service_mcp::McpApplication`, `service_mcp::serve_stdio` and
`service_mcp::streamable_http_router`, so those paths are a contract.

## Exceptions and debts

- **Checker exceptions:** None. `bearer_token` is in service-auth's
  application layer, which other contexts may use.
- **Name kept:** `McpApplication` reads like the DDD application layer
  although the trait belongs to the interfaces form. The name stays because
  sift implements the trait and a sift test asserts its path.
- **Debts:** `anyhow` in `HttpTransportConfig::new`,
  `HttpTransportConfig::from_env` and `serve_stdio` (these are not ports,
  so ADR D4 does not cover them).
