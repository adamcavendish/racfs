# RACFS Book

This directory contains the source for the **RACFS Book** — the main documentation for RACFS, including an introduction and a page for each plugin.

## Building and serving

From this directory (`docs/book/`):

```bash
mdbook build      # Output in build/
mdbook serve --open   # Serve at http://localhost:3000 with live reload
```

Install mdbook first if needed: `cargo install mdbook`.

## Structure

- `book.toml` — mdbook configuration
- `src/SUMMARY.md` — Table of contents
- `src/intro.md` — Introduction and quick start
- `src/plugins/` — One page per plugin (memfs, localfs, s3fs, etc.)
- `src/more.md` — Links to architecture, plugin development, and other docs
