# Release notes 0.6.0

This page lists the changes from `0.5.0` to `0.6.0`.

## Added

- openapi-codegen: a Managed profile. `generate_with_auth` with a `GenerationAuth` that holds a `ManagedAuth` emits a TypeScript, Python or Rust client that takes a CA path, accepts only `https:` URLs, reads the fixed token path `/var/run/secrets/kubernetes.io/serviceaccount/token` on every request, fails before any transport call when the token or the CA file is missing or empty, and never retries a 401 without the token. Output of a generation without Managed auth, and of `FileBearerAuth`, does not change (#263).
- faberline/lumen#109 uses the Managed profile after the next core tag.
