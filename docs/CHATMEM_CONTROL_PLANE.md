# ChatMem Control Plane

ChatMem V2 treats repository memory as a local-first control plane instead of a single MCP memory endpoint. The goal is to keep the current V1 tools working while adding richer surfaces for desktop review, automation, and future agent metadata exchange.

## Surfaces

### Desktop App

The desktop UI is the human-facing surface. It is where runs, artifacts, checkpoints, handoffs, and memory candidates can be reviewed together before anything is promoted or resumed.

### MCP

MCP remains the integration surface for host tools and other clients. The original five tools stay available:

- `get_repo_memory`
- `search_repo_history`
- `create_memory_candidate`
- `list_memory_candidates`
- `build_handoff_packet`

The V2 tools are additive:

- `list_active_runs`
- `list_run_artifacts`
- `create_checkpoint`
- `resume_from_checkpoint`

These tools expose the run timeline and checkpoint continuation flow without changing the existing V1 contract.

### A2A-Lite Metadata

The A2A-lite surface is a small metadata card that describes ChatMem as a local-first control plane. It is intentionally simple and does not add network transport or a distributed protocol layer.

The card is exposed from the Tauri layer and currently communicates:

- the product name
- the local-first description
- the skills ChatMem provides
- which local surfaces are available

## Compatibility

Compatibility is a design constraint, not a best effort.

- Existing V1 MCP clients keep working.
- The additive V2 tools do not replace the original five tools.
- The desktop app can use the new surfaces without requiring remote coordination.
- The metadata card is informational only and does not alter run, checkpoint, or handoff behavior.

## Practical Guidance

- Use V1 tools when you only need repository memory or a handoff packet.
- Use V2 tools when you need to inspect active runs, enumerate artifacts, or freeze and resume a checkpoint.
- Use the Agent Card when a client needs a compact description of ChatMem before deciding how to integrate with it.
