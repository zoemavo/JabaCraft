# rustcraft

`rustcraft` is an original voxel-sandbox game prototype written in Rust with
Bevy 0.19. The current milestone renders a biome-driven world from real chunked
voxel data without creating an entity for every block.

## Architecture

`src/main.rs` is deliberately small: it configures Bevy's window and composes
the top-level plugins. `src/lib.rs` exposes the reusable game modules; each
plugin owns its resources and future systems.

- `app` — application metadata shared across the shell.
- `game` — the `Loading`, `Playing`, and `Paused` lifecycle states.
- `scene` — player and lighting bootstrap for the playable 3D scene.
- `player` — kinematic player body, movement, controls, and child camera.
- `world` — composition root for block, chunk, generation, and meshing work.
- `block` — compact `BlockId` values and the centralized block-definition registry.
- `chunk` — bounds-checked 16³ block storage, dirty tracking, and loaded chunks.
- `coordinates` — signed world/chunk positions and validated local positions.
- `generation` — deterministic climate maps, biomes, terrain, caves, ores, trees, and player-relative chunk streaming.
- `meshing` — visible-face extraction, a procedural original texture atlas, and the `ChunkMeshingPlugin` render lifecycle.
- `interaction` — block targeting, breaking, and placing.
- `item` — distinct item IDs, definitions, block-item mappings, and stack rules.
- `inventory` — 9 hotbar slots, 27 storage slots, cursor-held stacks, and item movement rules.
- `persistence` — world/player save, load, and data migrations.
- `ui` — crosshair, persistent hotbar, and the separate modal inventory presentation.

Voxel blocks remain inside `ChunkStorage`. Rendering state is owned separately
by `ChunkRenderer`, whose `ChunkPos` mapping tracks one Bevy entity and one mesh
asset per rendered chunk. Dirty meshes are updated in place, while unloaded
chunks have both their entity and mesh asset removed. No Bevy entity or asset
handle is stored in `Chunk` or `ChunkStorage`.

The loaded world follows the player with a circular horizontal radius of eight
chunks. Vertically, the initial world bounds are fixed at `-64..192` blocks
(minimum inclusive, maximum exclusive), corresponding to chunk layers
`-4..=11`. This keeps terrain continuous across vertical chunk boundaries while
leaving room for future underground and elevated generation. Existing chunks
are retained, while chunks outside the window are unloaded instead of generating
an unbounded world.

The terrain generator exposes a conservative empty-chunk prediction for layers
entirely above the surface. Render-time voxel data remains authoritative:
fully air-filled chunks produce no geometry and own neither a render entity nor
a mesh asset.

Temperature and humidity use independent smooth noise maps sampled from seed and
world X/Z coordinates. Their climate values select `Plains`, `Forest`, `Desert`,
or `Rocky` per column. Biome height profiles are blended to avoid terrain cliffs;
surface blocks and deterministic tree density remain biome-specific. Sampling in
world space makes transitions continuous across chunk borders instead of forming
chunk-sized squares.

Trees are order-independent world features rather than chunk-owned decorations.
Each chunk evaluates candidate origins in a two-block margin around itself and
rasterizes only the `Wood`/`Leaves` voxels intersecting its own bounds. Forests
have the highest density, Plains receive occasional trees, and Desert/Rocky
receive none. Terrain-height checks require supported grass ground and reject
steep cliff margins; feature voxels never replace terrain blocks.

Coal and iron are generated as small deterministic 3D veins. Candidate cells,
vein centers, ellipsoid sizes, and irregular edges are pure functions of the
world seed and cell/voxel coordinates. Chunks scan a margin around their bounds,
so the same vein crosses chunk borders identically in any loading order. Coal is
more frequent and spans a wider depth range; iron is rarer and deeper. Ore only
replaces underground `Stone`, never `Air`, soil, or exposed rocky surfaces.

Caves use smooth 3D noise sampled directly in world coordinates, so a cave
continues naturally through every chunk boundary and remains independent of
generation order. `CaveSettings` exposes frequency, density threshold, and a
protected depth below the terrain surface. Carving only replaces underground
`Stone`/`Dirt` with `Air`; it runs after terrain fill and before ore/tree
features, preserving the surface and allowing later ore generation around cave
walls.

Missing chunks enter a distance-prioritized `ChunkGenerationQueue`. At most four
terrain jobs run concurrently by default in Bevy's `AsyncComputeTaskPool`, and each tracked chunk moves through
`Requested`, `Generating`, `Generated`, `Meshing`, and `Ready`. The lifecycle
registry deduplicates requests so an existing or queued chunk is never generated
twice. Worker tasks receive only immutable value inputs (`ChunkPos`, world seed,
and cave settings); completed voxel data is validated against the current
configuration and inserted into the main world on a later frame.

### Debug controls

- `W`, `A`, `S`, `D` — move relative to the camera direction.
- `Ctrl` — sprint while normal collision mode is active.
- `Space` — jump, only while grounded.
- `F4` — toggle optional debug noclip mode.
- In noclip: `WASD` follows the full camera view, `Space` ascends, and `Shift` descends.
- Mouse — rotate the first-person camera.
- Left mouse button — instantly break the selected breakable block. Holding it
  repeats at a debounced rate instead of once per rendered frame.
- Right mouse button — place the selected hotbar block against the hit
  face. Placement cannot overlap the player, replace a solid voxel, or leave
  the configured vertical world bounds.
- `1`–`9` or mouse wheel — select one of the nine hotbar slots. The selected
  block item resolves to a `BlockId` through `ItemRegistry` and loses one item on success.
- `E` — open or close the inventory; `Escape` also closes it.
- Inventory mouse controls: left-click takes, places, merges, or swaps a stack;
  right-click takes half or places one item.
- With inventory closed, `Escape` releases the cursor; left-click captures it again.

Walk, sprint, noclip speed, acceleration, deceleration, jump, gravity, terminal
velocity, collider dimensions, mouse sensitivity, and pitch limit are configured
through the `PlayerSettings` resource. Input is sampled before simulation, while
acceleration, gravity, integration, and collision run at a fixed 64 Hz physics
timestep independent of render FPS. Normal player movement uses
a `0.6 x 1.8` axis-aligned bounding box resolved independently along X, Y, and Z
against solid blocks in `ChunkStorage`. Collision queries cover only voxels
overlapping the player's bounds; substeps prevent ordinary movement from
tunneling through one-block walls. Missing async chunks conservatively suspend
movement until their voxel data arrives.

The interaction plugin performs a 3D DDA traversal from the player camera with
a default reach of five blocks. It reports the first non-air voxel, its
`BlockId`, hit face and axis-aligned normal, plus the previous empty cell for
future block placement. Unloaded chunks stop traversal so interaction cannot
pass through unknown world data.

The current target is shown with a slightly expanded 12-edge wireframe. It is
drawn through reusable Bevy gizmo buffers, avoiding z-fighting without spawning
per-frame entities or placing a material cube over the voxel.

Left-click breaking validates the current block through `BlockRegistry`, replaces
only breakable non-air voxels with `Air`, and uses `ChunkStorage::set_block` so
the changed chunk and any loaded face-neighbor at a chunk boundary are remeshed.

## Roadmap

1. Establish the Bevy application shell.
2. Add voxel/block data types and chunk storage.
3. Generate and render biome terrain with deterministic trees.
4. Add deterministic cave and ore generation (current milestone).
5. Add block interaction and a basic HUD.
6. Add persistence, lighting, and performance profiling.

## Run

Install the stable Rust toolchain, then run:

```powershell
cargo run
```

To validate the project without launching the window:

```powershell
cargo fmt --check
cargo check
cargo test
```

## Windows installer

Build the optimized executable and a per-user Windows installer with:

```powershell
.\scripts\build-installer.ps1
```

Artifacts are written to `dist/`. The installer copies the game to
`%LOCALAPPDATA%\Programs\rustcraft`, creates Start Menu and desktop shortcuts,
and registers an uninstaller in Windows Settings.
