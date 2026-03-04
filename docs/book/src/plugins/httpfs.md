# HttpFS

**HTTP client as a filesystem.** You write request parameters (URL, method, headers, body) to virtual files and read responses (status, headers, body) from other files. Useful for scripting HTTP calls or caching GET responses.

## Layout

Rough structure:

- **requests/** — Create a directory per request (e.g. `requests/my-id/`). Write `url`, `method`, optional `headers.json`, `body`, then write to `trigger` to send the request.
- **responses/** — After triggering, read `responses/my-id/status`, `headers.json`, `body`.
- **cache/** — Cached GET responses by URL (when caching is enabled).

## Config

No mandatory options; optional cache settings can be exposed via config. See crate docs for the exact layout and semantics.

## Crate

`racfs-plugin-httpfs`
