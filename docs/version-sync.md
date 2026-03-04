# Version synchronization strategy

This document describes how RACFS server, Rust crates, and client SDKs (Python, TypeScript) stay version-aligned for releases.

## Principle

**Single source of truth:** The [workspace version](https://github.com/adamcavendish/racfs/blob/master/Cargo.toml) in the repo root (`[workspace.package] version`) is the canonical API/SDK version. All publishable artifacts that speak to the RACFS REST API should use the same semantic version so that “racfs 0.2.0” means the same API surface across server, Python SDK, and TypeScript SDK.

## Semantic versioning commitment

RACFS **commits to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html)** for:

- **Workspace version** — The single version in `[workspace.package] version` governs server, CLI, and SDKs that speak the REST API.
- **Individual Rust crates** — When published to crates.io, each crate (`racfs-core`, `racfs-client`, `racfs-vfs`, `racfs-fuse`, `racfs-server`, `racfs-cli`) follows semver independently. A breaking change in one crate's public API requires a major bump for that crate; the workspace version may still be bumped according to the "When to bump" rules below for user-facing impact.

Compatibility expectations for public APIs are documented in [docs/api-review.md](api-review.md). Before a major release, review that document and ensure deprecations are communicated and migration paths exist.

## Deprecation policy

- **Marking:** Use Rust’s `#[deprecated(since = "X.Y.Z", note = "...")]` and document the replacement in the note (e.g. "Use `Client::builder().build(url)` instead").
- **Support window:** Deprecated APIs remain supported for at least one minor release. They may be removed in the next **major** version.
- **Changelog:** List deprecations under **Deprecated** in `CHANGELOG.md` when they are introduced.

## Version locations

| Artifact | File | Field |
|----------|------|--------|
| Rust workspace (server, client, fuse, etc.) | `Cargo.toml` (root) | `[workspace.package] version` |
| Python SDK | `sdks/python/pyproject.toml` | `[project] version` |
| TypeScript SDK | `sdks/typescript/package.json` | `"version"` |

Before a release, ensure all three match (e.g. `0.1.0` → `0.2.0`).

## When to bump

- **Major (x.0.0):** Breaking API or SDK changes (e.g. remove or rename endpoints, change request/response shapes in a non-backward-compatible way).
- **Minor (0.x.0):** New endpoints, new optional parameters, new SDK methods. Existing callers remain valid.
- **Patch (0.0.x):** Bug fixes, docs, behavior fixes that do not change the public API or SDK surface.

## Release checklist (manual until automated)

1. **Decide the new version** (e.g. `0.2.0`).
2. **Update versions** in:
   - Root `Cargo.toml`: `[workspace.package] version = "0.2.0"`
   - `sdks/python/pyproject.toml`: `version = "0.2.0"`
   - `sdks/typescript/package.json`: `"version": "0.2.0"`
3. **Update changelog** — Edit `CHANGELOG.md`: move items from `[Unreleased]` into a new `[X.Y.Z]` section with the release date, and add a compare link. See [Keep a Changelog](https://keepachangelog.com/).
4. **Tag** the repo: `git tag v0.2.0`.
5. **Publish** (when automated):
   - Rust: `cargo publish` for each crate (order: core → client → …).
   - Python: `cd sdks/python && pip install build && python -m build && twine upload dist/*`.
   - TypeScript: `cd sdks/typescript && npm publish`.

## Compatibility guarantee

- **SDK vs server:** A given SDK version is tested against the same server version. Older servers may lack new endpoints; newer servers should remain backward compatible for the same major version.
- **Pinning:** Users can pin `racfs==0.1.0` (Python) or `@racfs/sdk@0.1.0` (npm) to lock to a specific API version.

## Automation (future)

A release workflow (e.g. `.github/workflows/release.yml`) can:

- Read the workspace version from `Cargo.toml` or a `VERSION` file.
- Update `sdks/python/pyproject.toml` and `sdks/typescript/package.json` from that value.
- On tag push `v*`, build and publish Rust crates, Python package, and npm package.

Until then, the manual checklist above is the version synchronization strategy.
