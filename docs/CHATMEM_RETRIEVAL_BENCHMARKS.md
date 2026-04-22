# ChatMem Retrieval Benchmarks

ChatMem now has a small retrieval benchmark harness in `src-tauri/src/chatmem_memory/eval.rs`.
It measures whether `search_history` returns an expected memory title within the requested top-k.

Current benchmark scope:

- Hybrid retrieval over approved memory, episodes, conversations, and generated wiki pages.
- Local vector fallback from `chatmem-local-hash-v1` embeddings.
- Recall@k on deterministic fixture cases.

Run the benchmark fixture:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml chatmem_memory::eval
```

This is intentionally a starter benchmark, not a broad public leaderboard. The next useful step is
to add anonymized real project tasks with expected memory hits and track recall@3 plus wrong-top-1
cases across releases.
