const MANAGED_TOKEN_PATH = __MANAGED_TOKEN_PATH__;
const MANAGED_CA_PATH = __MANAGED_CA_PATH__;

async function readManagedFile(path: string, what: string): Promise<string> {
  const nodeProcess = (globalThis as any).process;
  if (!nodeProcess?.versions?.node) {
    throw new Error(`managed ${what} is unavailable in this runtime`);
  }
  const fsModule = "node:fs/promises";
  let raw: string;
  try {
    const { readFile } = (await import(fsModule)) as any;
    raw = await readFile(path, "utf8");
  } catch {
    throw new Error(`managed ${what} is unavailable`);
  }
  if (!raw.trim()) throw new Error(`managed ${what} is empty`);
  return raw;
}

// Runs before any transport call. A managed client has no anonymous mode:
// every failure here stops the request, and `request` sends exactly one
// transport call, so a 401 is never retried without the token.
async function prepareManaged(
  config: ClientConfig,
  url: URL,
): Promise<{ authorization: string; ca: string }> {
  if (url.protocol !== "https:") {
    throw new Error("managed clients accept only https: URLs");
  }
  const token = (await readManagedFile(MANAGED_TOKEN_PATH, "token")).trim();
  if (/[^\x21-\x7e]/.test(token)) throw new Error("managed token is invalid");
  const ca = await readManagedFile(config.caPath ?? MANAGED_CA_PATH, "CA bundle");
  return { authorization: `Bearer ${token}`, ca };
}

