# ProxyFS

**Proxy to another RACFS server.** Forwards all requests to a remote RACFS API. Use it to expose a second server under a path, aggregate multiple servers, or put a local path in front of a remote tree.

## Config

Requires the base URL of the remote server:

```toml
[mounts.remote]
path = "/remote"
fs_type = "proxyfs"
url = "http://localhost:9090"
```

Start the other server on the given port (e.g. 9090) to test.

## Behavior

- All `FileSystem` operations are translated to HTTP calls to the remote server.
- Paths under the mount are sent as paths to the remote API (e.g. `/remote/foo` → request to remote with path `/foo` or as configured).

## Crate

`racfs-plugin-proxyfs`
