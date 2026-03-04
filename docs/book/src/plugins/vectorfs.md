# VectorFS

**Vector database filesystem.** Stores documents as files and supports similarity search using vector embeddings. Good for RAG, semantic search, or document indexing.

## Layout

- **documents/** — Store documents as files (e.g. `documents/{id}.txt`). Writing a file indexes it (optionally using an embedding API).
- **index/** — Read-only: `count`, `status` (e.g. "ready" or "indexing").
- **search/** — Create a directory per query (e.g. `search/{query_id}/`). Write the query to `query.txt`, read `matches.txt` for ranked results (e.g. JSON).

## Config

Optional:

- **db_path** — Persistence path for the vector index.
- **embedding_url** — URL of the embedding API used to compute vectors.

See `examples/configs/vectorfs.toml` and `VectorConfig` in the crate.

## Crate

`racfs-plugin-vectorfs`
