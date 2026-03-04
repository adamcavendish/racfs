# Plugin Ecosystem

This document describes the RACFS plugin ecosystem: how plugins are discovered, how to publish and maintain third-party plugins, and how certification and community examples fit in. For implementing a plugin, see [Plugin Development](plugin-development.md).

## Plugin registry / marketplace concept

RACFS does not yet run a central plugin registry. The following describes a possible future direction and how to work today.

### Current model

- **Built-in plugins** live in the main repo (`crates/racfs-vfs/src/plugins/`): MemFS, LocalFS, DevFS, ProxyFS, StreamFS, HttpFS, S3FS, PgsFS, and others. The server mounts them by type name in config (e.g. `fs_type = "memfs"`).
- **Third-party plugins** are separate crates that implement `racfs_core::FileSystem`. To use them:
  1. Add the crate as a dependency of your server binary or a wrapper crate.
  2. Instantiate the plugin and call `MountableFS::mount(path, Arc::new(your_plugin))` (or extend server config to recognize a new `fs_type` and construct your plugin).

### Future registry concept

A registry could provide:

- **Discovery** — A list or index of plugins (e.g. by name, description, tags) with metadata and links to source or crates.
- **Version compatibility** — Which plugin versions are tested with which RACFS server versions (see [Version alignment](#version-alignment)).
- **Distribution** — Links to crates.io, GitHub releases, or prebuilt binaries.

Implementation options (not yet in place) include:

- A **curated list** in this repo (e.g. `docs/community-plugins.md`) or on the website.
- **crates.io** as the primary distribution: plugins publish as crates depending on `racfs-core`; users add them to their server’s `Cargo.toml`.
- A **config-driven discovery** layer (e.g. server config pointing at a manifest URL) for enterprise or private registries.

Until a registry exists, third-party plugins are distributed as crates or source and integrated by adding them as dependencies and mounting them in code or via extended server configuration.

## Third-party plugin guidelines

If you publish a plugin for others to use, follow these guidelines so it works well with the RACFS server and FUSE client.

### Version alignment

- **Pin `racfs-core`** to a version compatible with your target server (e.g. `racfs-core = "0.1"`). Prefer a minimum compatible version rather than a single exact version so patch updates work.
- Document which RACFS server (or version range) you test against (e.g. in the crate README or in [Version synchronization](version-sync.md)).

### Naming and packaging

- **Crate name:** Use a clear prefix to avoid clashes, e.g. `racfs-*` for official or endorsed plugins, or `mycompany-racfs-*` for third-party.
- **Binary vs library:** Publish as a **library** crate that exports a type implementing `FileSystem` (and optionally `PluginMetrics`). Users embed it in their server; they do not run your crate as a standalone binary for the server.
- **License:** Prefer MIT or Apache-2.0 so the RACFS community can reuse and integrate easily.

### Implementation checklist

- Implement all required `FileSystem` methods; return `FSError::NotSupported` or similar for operations you do not support.
- Use `Result<T, FSError>` and standard `FSError` variants so the server and FUSE layer can map errors consistently.
- Optionally implement `PluginMetrics` (see [Plugin-specific metrics](https://github.com/adamcavendish/racfs/blob/master/ROADMAP.md)) and document any extra configuration (e.g. env vars, config file).
- Add tests (unit and, if possible, integration against a real server or a test harness).

### Publishing

- **crates.io:** Publish the crate so users can add it with `racfs-myplugin = "0.1"`. Document how to mount it (e.g. “Add to your server binary and mount at `/mymount`”).
- **Source:** If you only distribute via Git, document the dependency path and any build/feature flags.

## Plugin certification process

Certification is a **future** concept to indicate that a plugin has been checked for compatibility and best practices. Nothing is automated today; the following is a target shape.

- **Eligibility** — Plugin is publicly available (crate or source), implements `FileSystem`, and has a README and version alignment note.
- **Checks (candidate)**  
  - Builds with a stated `racfs-core` (or server) version.  
  - Passes a shared test suite or a documented compatibility test (e.g. basic create/read/write/stat/readdir).  
  - Follows [third-party plugin guidelines](#third-party-plugin-guidelines) (errors, naming, versioning).
- **Outcome** — “Certified for RACFS 0.x” could be a badge or a listing in a curated registry. Today, maintainers can self-certify by documenting the above in the plugin repo.

No formal certification pipeline exists yet; this section describes a possible process once the project introduces it.

## Community plugin examples

These are starting points for building your own plugin and seeing how others structure code.

### In this repository

- **Custom plugin example** — `crates/racfs-vfs/examples/custom_plugin.rs` shows a minimal in-memory plugin and how to drive it from a small binary.
- **Built-in plugins** — Implementations to use as reference:
  - **MemFS** — `crates/racfs-vfs/src/plugins/memfs.rs`
  - **LocalFS** — `crates/racfs-vfs/src/plugins/localfs.rs`
  - **HttpFS** — `crates/racfs-vfs/src/plugins/httpfs.rs`
  - **StreamFS** — `crates/racfs-vfs/src/plugins/streamfs.rs`

### External plugins

When third-party plugins are published (e.g. on crates.io or listed in a future registry), they can be linked here or in a dedicated community-plugins list. For now, search crates.io for crates depending on `racfs-core` to find community plugins.

### Contributing a plugin

If you build a plugin you want to share:

1. Publish it (crate or open source) and add a short note in your README: “RACFS plugin — compatible with racfs-core 0.x / server 0.x.”
2. Open an issue or PR in the RACFS repo to add it to a community list once one exists (e.g. `docs/community-plugins.md` or the website).

## See also

- [Plugin Development](plugin-development.md) — Implement the `FileSystem` trait.
- [Architecture](architecture.md) — How the server and VFS use plugins.
- [Version synchronization](version-sync.md) — Aligning plugin and server versions for releases.
