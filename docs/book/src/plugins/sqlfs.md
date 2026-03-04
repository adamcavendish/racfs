# SQLfs

**SQLite-backed filesystem.** Stores files and directory structure in a SQLite database. Good for single-file persistence, embedded use, or when you want SQL-backed metadata.

## Config

Configure the database path (and any SQLite options your build supports). See server config schema and example configs for the exact keys.

## Behavior

- Implements full `FileSystem`: create, read, write, mkdir, read_dir, rename, remove, chmod.
- Data and metadata are stored in SQLite tables.
- Suitable for moderate-sized trees and single-server deployments.

## Crate

`racfs-plugin-sqlfs`
