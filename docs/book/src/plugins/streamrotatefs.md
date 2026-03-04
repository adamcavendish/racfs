# StreamRotateFS

**Rotating log files filesystem.** Writes go to a “current” file; when it reaches a size or count limit, it rotates (e.g. renames to a numbered file and starts a new current). Good for log aggregation or bounded-size streams.

## Config

Options (see `RotateConfig` in the crate):

- **max_size** — Max bytes per file before rotation.
- **max_files** — Max number of rotated files to keep.

## Behavior

- Write to a path (e.g. `/current`); the plugin manages rotation internally.
- Read from the current or from rotated segments as defined by the plugin layout.
- Implements create, read, write, and directory listing for the rotated set.

## Crate

`racfs-plugin-streamrotatefs`
