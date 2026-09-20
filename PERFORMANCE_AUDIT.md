# Performance audit

Audited on 2026-09-20 against Bevy 0.19 runtime paths. This is a code-path audit backed by release benchmarks and the existing F3 timing counters for terrain generation, meshing, and chunk loading.

| Area | Finding | Result |
| --- | --- | --- |
| Chunk snapshots | Every meshing task deep-cloned up to 27 dense chunks (up to 216 KiB of voxel copies). Save capture also copied every modified chunk on the render thread. | Dense voxel storage now uses `Arc<[BlockId]>` with copy-on-write edits. Worker and save snapshots share immutable allocations safely. |
| Streaming | The default radius rebuilt a `HashSet` of 3,152 required positions and scanned the full lifecycle set every frame. | The required window is cached and rebuilt only when the player changes chunk or generation inputs change. |
| Priority queues | Generation and meshing heaps were rebuilt each frame, including while the player stood still. | Heaps persist. They are reprioritized for new work, a changed player chunk, or a meaningful view-direction change. |
| Mesh discovery | Every frame scanned every loaded chunk to rediscover dirty mesh candidates. | `ChunkStorage` exposes a geometry epoch; the full scan now runs only after insert, unload, or voxel edit. |
| Mesher allocations | Greedy meshing allocated one mask for every face direction. | One fixed-size stack mask is reused for all six directions, removing those heap allocations. |
| Render cleanup | Every frame built a second `HashSet` containing all loaded chunks. | Cleanup now runs only after the storage geometry epoch changes and checks `ChunkStorage` directly, removing the allocation and the steady-state scan. |
| ECS/UI work | Settings, audio sinks, survival icons, and closed inventory views received redundant writes or scans every frame. | Stable settings and HUD values short-circuit; closed inventory panels skip their slot and crafting passes. |
| Entity count | Voxel data is dense chunk data. A chunk creates at most one opaque mesh entity and one water mesh entity. | There is no entity per voxel. A regression test verifies that 4,096 solid voxels create exactly one chunk entity and one mesh asset. |
| Mesh/assets | Dirty chunks reuse existing entities and `Mesh` handles; empty layers and unloaded chunks remove both entity and asset. | Existing rebuild/unload/mixed-layer tests cover the lifecycle; no asset leak was found. |
| Terrain generation | Noise, caves, ores, and features are CPU-heavy, but execute in the bounded `AsyncComputeTaskPool` queue. | Kept off the main thread; F3 records worker duration. No terrain algorithm rewrite was justified. |
| Save I/O | Serialization and atomic temp-file/fsync/rename run on `IoTaskPool`, with one active job and one coalesced follow-up. The archive is monolithic and scales with all modified chunks. | Render-thread voxel copies were removed. A region/incremental format remains a future option only if large-world save measurements justify the format change. |
| Hash maps | Face visibility previously performed a chunk-map lookup for every tested voxel face, including neighbors inside the same chunk. Collision and raycast perform only a small number of world lookups; lighting already overlays chunks into a bounded dense local map. | Interior face checks now read the current chunk array directly, reserving hash lookup for the six chunk boundaries. A full chunk drops from 24,576 to 1,536 chunk-map lookups during visibility testing. Replacing the maps or hasher was not justified. |

The bounded light volume remains the largest allocation inside a non-empty mesh job: it covers the 3x3x3 neighborhood plus propagation padding. It is worker-local and limited by `max_meshing_tasks`; changing its lighting behavior would risk visible regressions, so this audit leaves it intact and observable through the F3 meshing timings.

Release benchmark commands:

```text
cargo test --release chunk_snapshot_benchmark -- --ignored --nocapture
cargo test --release greedy_benchmark -- --ignored --nocapture
```

On the audit machine, an 8 KiB chunk snapshot averaged 0.152 microseconds for a deep copy and 0.010 microseconds for the shared clone over 20,000 runs (about 15x faster). Greedy geometry retained its main reductions: a solid fixture went from 6,144 to 1,044 vertices, and a mixed fixture from 3,200 to 1,440. Checkerboard geometry correctly cannot merge. Timing values are machine-dependent; geometry counts are deterministic.
