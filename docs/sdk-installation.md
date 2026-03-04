# SDK Installation

RACFS provides client SDKs for Python and TypeScript/JavaScript so you can call the RACFS REST API from your application.

## Overview

| SDK        | Package        | Repo path        |
|-----------|----------------|-------------------|
| Python    | `racfs`        | `sdks/python/`    |
| TypeScript| `@racfs/sdk`   | `sdks/typescript/`|

## When published

Once the SDKs are published to PyPI and npm:

```bash
# Python
pip install racfs

# Node / TypeScript
npm install @racfs/sdk
```

See [ROADMAP](../ROADMAP.md) for publish status (0.1.0 scope).

## Development / unreleased installs

### Python

From a local clone of the RACFS repo:

```bash
pip install -e sdks/python
```

From git (no clone):

```bash
pip install git+https://github.com/adamcavendish/racfs.git#subdirectory=sdks/python
```

### TypeScript / JavaScript

From a local clone:

```bash
cd sdks/typescript
npm install
npm run build
```

Then in your application either:

- Add a dependency in `package.json`: `"@racfs/sdk": "file:../../path/to/racfs/sdks/typescript"`, or  
- Use `npm link`: from `sdks/typescript` run `npm link`, then in your app run `npm link @racfs/sdk`.

## Version alignment

SDK versions are kept in sync with the RACFS server API. The SDKs target the current REST API (e.g. `/api/v1/...`). For the full version synchronization strategy (when to bump, where versions live, release checklist), see **[Version synchronization](version-sync.md)**.

## Links

- [Python SDK README](../sdks/python/README.md) – API reference and examples  
- [TypeScript SDK README](../sdks/typescript/README.md) – API reference and examples  
- [RACFS server](https://github.com/adamcavendish/racfs) – Run a server with `cargo run -p racfs-server`

## Publishing to PyPI and npm

Publishing is planned for 0.1.0 (see [ROADMAP](../ROADMAP.md)). Use the steps below when releasing.

### Python (PyPI)

1. Bump version in `sdks/python/pyproject.toml` (and any `__version__` in code) to match the release (e.g. `0.1.0`).
2. From the repo root, build the package:  
   `cd sdks/python && python -m build`
3. Upload to PyPI (requires an account and token):  
   `python -m twine upload dist/*`  
   For Test PyPI first: `python -m twine upload --repository testpypi dist/*`
4. Install and verify:  
   `pip install racfs` (after publish) or `pip install --index-url https://test.pypi.org/simple/ racfs` (Test PyPI).

### TypeScript / JavaScript (npm)

1. Bump version in `sdks/typescript/package.json` to match the release (e.g. `0.1.0`).
2. From the package directory:  
   `cd sdks/typescript && npm run build && npm publish`  
   For a scoped package like `@racfs/sdk`, ensure `publishConfig` is set in `package.json` if publishing to the public registry.
3. To publish under a scope you must have npm auth configured:  
   `npm login` and ensure the scope is allowed for your account.

Keep SDK versions aligned with the RACFS server release; see [Version synchronization](version-sync.md).
