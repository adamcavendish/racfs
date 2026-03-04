# QueueFS

**Message queue as a filesystem.** Directories are queues; you push by writing files, and consume via head/tail and acknowledgment files (e.g. `.ack`).

## Layout and usage

- Create a directory to create a queue (e.g. `mkdir /queues/myqueue`).
- Write a “message” by creating/writing a file under that directory.
- Consumers read from the head (or tail) of the queue; acknowledging is done via a special file (e.g. `.ack`) so messages are marked consumed or removed.

Exact semantics (naming, ordering, ack format) are defined in the plugin; see crate docs and tests.

## Config

No extra options in the typical config:

```toml
[mounts.queues]
path = "/queues"
fs_type = "queuefs"
```

## Crate

`racfs-plugin-queuefs`
