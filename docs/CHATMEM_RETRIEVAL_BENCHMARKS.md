# ChatMem Retrieval Benchmarks

ChatMem now has a small retrieval benchmark harness in `src-tauri/src/chatmem_memory/eval.rs`.
It measures whether `search_history` returns an expected memory title within the requested top-k.

Current benchmark scope:

- Hybrid retrieval over approved memory, episodes, conversations, and generated wiki pages.
- Local vector fallback from `chatmem-local-hash-v1` embeddings.
- Optional OpenAI-compatible embedding provider, stored side-by-side with the local fallback in
  `document_embeddings` using `(doc_id, embedding_model)` as the vector key.
- Recall@k on deterministic fixture cases.

Run the benchmark fixture:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml chatmem_memory::eval
```

This is intentionally a starter benchmark, not a broad public leaderboard. The next useful step is
to add anonymized real project tasks with expected memory hits and track recall@3 plus wrong-top-1
cases across releases.

## Real Embedding Provider

Set these environment variables before starting ChatMem or the MCP server:

```powershell
$env:CHATMEM_EMBEDDING_PROVIDER = "openai-compatible"
$env:CHATMEM_EMBEDDING_BASE_URL = "https://api.openai.com/v1"
$env:CHATMEM_EMBEDDING_MODEL = "text-embedding-3-small"
$env:CHATMEM_EMBEDDING_DIMENSIONS = "1536"
$env:CHATMEM_EMBEDDING_API_KEY = "<your key>"
```

Then call `rebuild_repo_embeddings` from MCP or the Tauri command surface for the target repo. Search
will use the configured provider and keep local hash vectors as a fallback if the provider is unavailable.
