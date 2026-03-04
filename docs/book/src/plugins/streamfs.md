# StreamFS

**Streaming data filesystem.** Provides named streams you can append to and read from, with optional compression and configurable buffer/history size.

## Config

Optional parameters (see example configs):

- **buffer_size** — Size of the in-memory buffer per stream.
- **history_size** — How much history to keep.
- **max_streams** — Maximum number of streams.

```toml
[mounts.streams]
path = "/streams"
fs_type = "streamfs"
buffer_size = 65536
history_size = 10
max_streams = 100
```

## Behavior

- Create or open a “stream” (file path); write appends to the stream, read returns data from the stream (with optional decompression).
- Useful for logs, event streams, or real-time data that multiple readers can consume.

## Crate

`racfs-plugin-streamfs`
