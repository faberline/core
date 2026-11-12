export interface ClientConfig {
  baseUrl: string;
  fetch?: typeof fetch;
  headers?: Record<string, string>;
  /**
   * Verify the server against a private CA instead of the public roots. Needs a
   * `fetch` that carries it -- see `privateCaFetch`.
   */
  trust?: PrivateTrust;
  transport?: TransportPolicy;
  /**
   * Epic #1296 POST-twin fallback: when true, every generated `QUERY`
   * operation is sent as `POST` against its documented twin path instead of
   * the HTTP `QUERY` method (RFC 10008). Off by default.
   */
  usePostFallback?: boolean;
}


export interface PrivateTrust {
  /** Path to the PEM bundle holding the CA that signed the server's leaf. */
  caBundle: string;
  /**
   * The DNS name the leaf must assert. The base URL has to address this same
   * name: a client that verified one name while addressing another would be
   * checking a certificate it never actually relies on.
   */
  serverName: string;
}

export interface TransportPolicy {
  targetConcurrency?: number;
  maxConnections?: number;
  maxInFlightPerOrigin?: number;
  maxKeepaliveConnections?: number;
  keepaliveExpiryMs?: number;
  poolTimeoutMs?: number;
}

const DEFAULT_MAX_CONNECTIONS = 128;
const DEFAULT_MAX_KEEPALIVE_CONNECTIONS = 16;
const DEFAULT_KEEPALIVE_EXPIRY_MS = 5000;
const DEFAULT_POOL_TIMEOUT_MS = 5000;

export function recommendedH2Connections(concurrency: number, parallelism = 1): number {
  const cap = Math.max(1, parallelism);
  if (concurrency <= 2) return 1;
  return Math.max(1, Math.min(cap, Math.ceil(Math.log(concurrency))));
}

class Admission {
  private active = 0;
  private readonly waiters: Array<() => void> = [];
  constructor(private readonly maxInFlight: number) {}

  async acquire(timeoutMs: number): Promise<() => void> {
    if (this.active < this.maxInFlight) {
      this.active += 1;
      return () => this.release();
    }
    return new Promise((resolve, reject) => {
      let done = false;
      let timer: ReturnType<typeof setTimeout>;
      const wake = () => {
        if (done) return;
        done = true;
        clearTimeout(timer);
        this.active += 1;
        resolve(() => this.release());
      };
      timer = setTimeout(() => {
        if (done) return;
        done = true;
        const index = this.waiters.indexOf(wake);
        if (index >= 0) this.waiters.splice(index, 1);
        reject(new Error(`pool timeout after ${timeoutMs}ms`));
      }, timeoutMs);
      this.waiters.push(wake);
    });
  }

  private release(): void {
    this.active = Math.max(0, this.active - 1);
    const wake = this.waiters.shift();
    if (wake) wake();
  }
}

const admissions = new WeakMap<ClientConfig, Map<string, Admission>>();

function policy(config: ClientConfig): Required<TransportPolicy> {
  const p = config.transport ?? {};
  const maxConnections = Math.max(1, p.maxConnections ?? DEFAULT_MAX_CONNECTIONS);
  return {
    targetConcurrency: Math.max(1, p.targetConcurrency ?? maxConnections),
    maxConnections,
    maxInFlightPerOrigin: Math.max(1, p.maxInFlightPerOrigin ?? p.targetConcurrency ?? maxConnections),
    maxKeepaliveConnections: Math.max(0, p.maxKeepaliveConnections ?? DEFAULT_MAX_KEEPALIVE_CONNECTIONS),
    keepaliveExpiryMs: Math.max(0, p.keepaliveExpiryMs ?? DEFAULT_KEEPALIVE_EXPIRY_MS),
    poolTimeoutMs: Math.max(0, p.poolTimeoutMs ?? DEFAULT_POOL_TIMEOUT_MS),
  };
}

async function acquireAdmission(config: ClientConfig, url: URL): Promise<() => void> {
  const p = policy(config);
  let byOrigin = admissions.get(config);
  if (!byOrigin) {
    byOrigin = new Map();
    admissions.set(config, byOrigin);
  }
  const key = url.origin;
  let admission = byOrigin.get(key);
  if (!admission) {
    admission = new Admission(p.maxInFlightPerOrigin);
    byOrigin.set(key, admission);
  }
  return admission.acquire(p.poolTimeoutMs);
}

export interface RequestArgs {
  method: string;
  path: string;
  query?: Record<string, unknown>;
  body?: unknown;
  headers?: Record<string, string>;
  expectBody?: boolean;
}


/**
 * A private trust anchor is only meaningful if the transport actually consults
 * it, and if the name being verified is the name being addressed.
 *
 * Neither can be arranged after the fact, so both are checked before the first
 * byte goes out. There is deliberately no option to skip verification: when the
 * server cannot be verified, the fix is the anchor or the name -- not the check.
 */
function assertTrustIsHonoured(config: ClientConfig, carrier: unknown): void {
  const trust = config.trust;
  if (!trust) return;
  const addressed = new URL(config.baseUrl).hostname;
  if (addressed !== trust.serverName) {
    throw new Error(
      `this client would verify ${trust.serverName} while addressing ${addressed}; ` +
        "point the base URL at the name the certificate asserts",
    );
  }
  if (!carrier) {
    throw new Error(
      "config.trust needs a transport that carries it: build one with " +
        "privateCaFetch(config.trust) and pass it as config.fetch",
    );
  }
}

/**
 * A `fetch` that verifies the server against `trust.caBundle` and nothing else.
 *
 * The public roots are not consulted: a private trust domain merely *added* to
 * the public set still lets any public CA certify this name.
 *
 * Node only, and a function you call rather than something the runtime arranges
 * itself, because a browser has no way to add a trust anchor -- there the
 * anchor has to be installed in the platform store instead.
 */
export async function privateCaFetch(trust: PrivateTrust): Promise<typeof fetch> {
  // Non-literal specifiers so a browser bundle never type-resolves these.
  const fsModule = "node:fs";
  const undiciModule = "undici";
  const { readFileSync } = (await import(fsModule)) as any;
  const { Agent } = (await import(undiciModule)) as any;
  const dispatcher = new Agent({
    connect: { ca: readFileSync(trust.caBundle, "utf8"), servername: trust.serverName },
  });
  return ((input: any, init?: any) =>
    fetch(input, { ...init, dispatcher })) as unknown as typeof fetch;
}

export async function request<T>(config: ClientConfig, args: RequestArgs): Promise<T> {
  const doFetch = config.fetch ?? fetch;
  assertTrustIsHonoured(config, config.fetch);
  const url = new URL(config.baseUrl + args.path);
  if (args.query) {
    for (const [key, value] of Object.entries(args.query)) {
      if (value !== undefined && value !== null) {
        url.searchParams.set(key, String(value));
      }
    }
  }
  const release = await acquireAdmission(config, url);
  try {
    const response = await doFetch(url.toString(), {
      method: args.method,
      headers: { "Content-Type": "application/json", ...config.headers, ...args.headers },
      body: args.body !== undefined ? JSON.stringify(args.body) : undefined,
    });
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    if (args.expectBody === false || response.status === 204) {
      return undefined as T;
    }
    return (await response.json()) as T;
  } finally {
    release();
  }
}

export type NdjsonItem<S> = S extends AsyncIterable<infer U> ? U : never;

export interface NdjsonOptions {
  /** Abort the request and the stream. */
  signal?: AbortSignal;
}

export interface NdjsonRequestArgs extends RequestArgs {
  /** Request body sent as NDJSON, one record per line, pulled lazily. */
  ndjsonBody?: AsyncIterable<unknown>;
}

export class NdjsonParseError extends Error {
  readonly line: number;
  constructor(line: number) {
    super(`malformed NDJSON record at line ${line}`);
    this.name = "NdjsonParseError";
    this.line = line;
  }
}

async function* encodeNdjson(items: AsyncIterable<unknown>): AsyncGenerator<Uint8Array> {
  const encoder = new TextEncoder();
  for await (const item of items) {
    yield encoder.encode(JSON.stringify(item) + "\n");
  }
}

async function* decodeNdjson<T>(chunks: AsyncIterable<Uint8Array>): AsyncGenerator<T> {
  const decoder = new TextDecoder();
  let pending = "";
  let line = 0;
  const parse = (text: string): T => {
    try {
      return JSON.parse(text) as T;
    } catch {
      throw new NdjsonParseError(line);
    }
  };
  for await (const chunk of chunks) {
    pending += decoder.decode(chunk, { stream: true });
    let end = pending.indexOf("\n");
    while (end >= 0) {
      const text = pending.slice(0, end).trim();
      pending = pending.slice(end + 1);
      line += 1;
      if (text) yield parse(text);
      end = pending.indexOf("\n");
    }
  }
  pending += decoder.decode();
  const rest = pending.trim();
  if (rest) {
    line += 1;
    yield parse(rest);
  }
}

async function* readChunks(body: ReadableStream<Uint8Array>): AsyncGenerator<Uint8Array> {
  const reader = body.getReader();
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) return;
      if (value) yield value;
    }
  } finally {
    reader.cancel().catch(() => undefined);
  }
}

function toReadableStream(chunks: AsyncGenerator<Uint8Array>): ReadableStream<Uint8Array> {
  return new ReadableStream<Uint8Array>({
    async pull(controller) {
      const { done, value } = await chunks.next();
      if (done) controller.close();
      else controller.enqueue(value);
    },
    async cancel() {
      await chunks.return(undefined);
    },
  });
}

async function sendNdjson(
  config: ClientConfig,
  args: NdjsonRequestArgs,
  options: NdjsonOptions,
  accept: string,
): Promise<{ response: Response; release: () => void }> {
  const doFetch = config.fetch ?? fetch;
  assertTrustIsHonoured(config, config.fetch);
  const url = new URL(config.baseUrl + args.path);
  if (args.query) {
    for (const [key, value] of Object.entries(args.query)) {
      if (value !== undefined && value !== null) {
        url.searchParams.set(key, String(value));
      }
    }
  }
  const headers = { "Content-Type": args.ndjsonBody ? "application/x-ndjson" : "application/json", Accept: accept, ...config.headers, ...args.headers };
  const release = await acquireAdmission(config, url);
  try {
    const init: Record<string, unknown> = { method: args.method, headers, signal: options.signal };
    if (args.ndjsonBody) {
      init.body = toReadableStream(encodeNdjson(args.ndjsonBody));
      init.duplex = "half";
    } else if (args.body !== undefined) {
      init.body = JSON.stringify(args.body);
    }
    const response = await doFetch(url.toString(), init as RequestInit);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    return { response, release };
  } catch (error) {
    release();
    throw error;
  }
}

export async function requestNdjsonUpload<T>(
  config: ClientConfig,
  args: NdjsonRequestArgs,
  options: NdjsonOptions = {},
): Promise<T> {
  const { response, release } = await sendNdjson(config, args, options, "application/json");
  try {
    if (args.expectBody === false || response.status === 204) {
      return undefined as T;
    }
    return (await response.json()) as T;
  } finally {
    release();
  }
}

export async function* requestNdjsonStream<T>(
  config: ClientConfig,
  args: NdjsonRequestArgs,
  options: NdjsonOptions = {},
): AsyncGenerator<T> {
  const { response, release } = await sendNdjson(config, args, options, "application/x-ndjson");
  try {
    if (!response.body) throw new Error("response has no body");
    yield* decodeNdjson<T>(readChunks(response.body));
  } finally {
    release();
  }
}
