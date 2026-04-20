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

Checkpoint creation also remains available from the earlier checkpoint phase:

- `create_checkpoint`

Task 6 adds the new additive control-plane tools:

- `list_active_runs`
- `list_run_artifacts`
- `resume_from_checkpoint`

Those additions expose the run timeline and checkpoint continuation flow without changing the existing V1 contract or re-defining pre-existing checkpoint creation.

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
- The additive Task 6 tools do not replace the original five tools.
- `create_checkpoint` remains part of the checkpoint flow that predated Task 6.
- The desktop app can use the new surfaces without requiring remote coordination.
- The metadata card is informational only and does not alter run, checkpoint, or handoff behavior.

## Practical Guidance

- Use V1 tools when you only need repository memory or a handoff packet.
- Use `create_checkpoint` when you want to freeze the current state into a checkpoint.
- Use the Task 6 additive tools when you need to inspect active runs, enumerate artifacts, or resume from a checkpoint.
- Use the Agent Card when a client needs a compact description of ChatMem before deciding how to integrate with it.
