<div align="center">

# JabaCraft

Playable voxel sandbox MVP built with Rust and Bevy 0.19.

</div>

## Features

- Main menu with new-world creation, optional `u64` seed, save-slot loading, and quit.
- Procedural voxel terrain with biomes, trees, caves, ores, water, and day/night lighting.
- Chunk streaming with bounded asynchronous generation and meshing queues.
- Walking, sprinting, jumping, voxel collision, swimming, block breaking and placement.
- Dropped items, hotbar, inventory, basic crafting, typed tools/weapons, pause menu and live settings.
- Autosave, manual save, atomic world files, and restoration of edited chunks after restart.
- Stylized sky, sun, ambient fill, soft shadows, fog, and underwater color/fog treatment.
- F3 diagnostics for frame timing, chunk queues, mesh geometry, target block, biome, and profiling.

## Controls

| Input | Action |
| --- | --- |
| `WASD` | Move |
| `Ctrl` | Sprint |
| `Space` | Jump; swim upward while in water |
| `Shift` | Descend while noclip is enabled |
| Mouse | Look around |
| Left mouse | Break the targeted block |
| Right mouse | Place the selected block |
| `E` | Open/close inventory |
| `1`–`9`, mouse wheel | Select hotbar slot |
| `F3` | Toggle debug overlay |
| `F4` | Toggle noclip |
| `F5` | Save immediately |
| `Esc` | Pause; close inventory first when it is open |

## Build and run

Install a current stable Rust toolchain and the Linux graphics/audio development
libraries required by Bevy. From the repository root:

```bash
cargo run
```

For a production build:

```bash
cargo build --release
./target/release/rustcraft
```

Run the release binary with the repository as its working directory so Bevy can
find `assets/`. The automated checks are:

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## Architecture

The application is divided into small Bevy plugins: `game` owns state and menus,
`generation` owns deterministic terrain and streaming, `meshing` owns worker mesh
jobs and render assets, `chunk` owns voxel storage, `player` owns fixed-step
movement/collision/water state, `interaction` owns raycasts and block edits,
`inventory`/`item`/`crafting` own gameplay data, and `persistence` owns save jobs.

Voxel blocks are stored in dense chunk arrays. There is no entity per voxel: a
non-empty chunk has at most one opaque mesh entity and one water mesh entity.
Workers receive immutable 3×3×3 snapshots; only the main world creates or updates
Bevy assets. Mesh revisions reject stale worker results. Generation follows
`Requested → Generating → Generated → Meshing → Ready` with bounded task and
upload budgets.

🇺🇸: hardcore open sauce mancrouft. It supports Linux, Windows, and possibly Mac (haven't tested it).

🇷🇺: жоский опен сос манкруфт. Поддерживает линукс, виндус и может быть мак (не тестил)

## World appearance

The world uses a lightweight procedural gradient cubemap through Bevy's
`Skybox`, with a muted sun disc and day/night palettes. Physical atmospheric
scattering is deliberately omitted to keep the voxel palette predictable and
avoid an additional atmosphere rendering pass.

The matte terrain receives a restrained directional sun and broad ambient
fill. Shadows use two 1024px cascades (20/80 blocks), a 20% cascade overlap,
and Gaussian filtering without temporal accumulation. Fog blends from 48 to
128 blocks and follows the horizon palette. There is no bloom, specular glare,
contact-shadow pass, or volumetric fog. Baked voxel occlusion remains in place;
partial skylight and face contrast are softened to avoid double-darkening.

Tune the light, sky, fog, and shadow budget in `src/scene/environment.rs`.
The camera's shadow filter is in `src/scene/mod.rs`; terrain material and baked
light response are in `src/meshing/mod.rs` and `src/meshing/lighting.rs`.

## Water

World generation fills columns above low terrain with stationary water up to
`SEA_LEVEL = 8` (`src/generation/terrain.rs`). Water voxels end at Y=7, so their
top surface lies at Y=8. Underground caves are not flooded, and trees do not
spawn on submerged ground.

Water is transparent and non-solid. `PlayerInWater` tracks the fraction of the
player's body inside actual water voxels, including partial/full immersion.
In water movement is capped at 2.2 blocks/s, gravity drops to 4 blocks/s²,
and sinking is capped at 2.5 blocks/s. Hold Space to swim up at up to 3 blocks/s.
These values live in `PlayerSettings`. Leaving water restores ordinary movement;
noclip keeps its own flight controls. There is no fluid simulation or oxygen system.

When the camera goes under water, a short transition adds blue-green distance
fog (14-block visibility), reduces exposure and cools the scene colors. The
effect follows camera immersion independently of the body and clears on surfacing.
The HUD stays readable. Existing health rules are unchanged.

## Tools and weapons

The item registry includes wooden, stone, and iron pickaxes, axes, shovels, and
swords. Every tool is unstackable and has one `ToolDefinition` containing its
category and durability plus `ToolProperties` for mining speed, attack damage,
attack speed, and harvest tier. Pickaxes accelerate stone and ores, axes
accelerate logs, shovels accelerate grass/dirt/sand, and swords accelerate
leaves. Harvest-tier checks currently matter for stone, coal ore, and iron ore.

All twelve tool and weapon sprites come unchanged from the official Faithful
32x Java pack; this atlas contains no temporary tool placeholders. Durability
wear and melee combat are not implemented yet, so durability and attack values
are registry data for those follow-up systems. Items already work in inventory,
hotbar, dropped-item, tooltip, and crafting data paths.

Each chunk has independent opaque/cutout and water mesh assets and entities.
Leaves retain the cutout material; water uses the Faithful atlas water tile on
a separate matte alpha-blended, double-sided material and does not cast shadows.
Shared water faces are culled
across all chunk boundaries, while solid shore faces remain visible through
the water. Both layers are rebuilt and released with the chunk lifecycle.
Transparency uses Bevy's normal mesh sorting, not per-triangle sorting or OIT.

## Debug overlay

Press `F3` while playing to toggle the debug overlay. It reports FPS and frame
time, player/chunk coordinates, view direction, biome and targeted block, loaded
and visible chunk counts, generation/meshing queue pressure, mesh geometry, and
estimated CPU memory held by voxels and mesh buffers. It also shows cumulative
average/last/maximum CPU timings for terrain generation, worker meshing, and
main-world chunk loading. Expensive overlay counters refresh four times per
second, and the UI entity and frame sampler are absent while the overlay is off.

## Main menu

The game starts in MainMenu, with Play, Create World and Quit. Play lists save
slots under `saves/`; select a world and press Load. The list scrolls with the
mouse wheel. Create World accepts a slot name and an optional unsigned 64-bit
seed; leaving it blank chooses a seed automatically. Use Tab to switch fields.
Existing slots cannot be overwritten by Create.

Selection transitions through Loading, which restores metadata and waits for
nearby chunks before enabling Playing, physics, the captured cursor and HUD.
A load error returns to the menu with an explanation and preserves the save.
The first playable frame requests a save so new worlds appear in the list.
Buttons, panorama and pixel font come from the same Faithful revision as the
block textures. The font is derived from its ASCII bitmap; the reproducible
converter is `tools/import_menu_font.py` (Pillow and fonttools).

## Pause and preferences

Escape pauses gameplay and opens Resume, Settings, Save, Main Menu and Quit.
With inventory open, the first Escape closes inventory. Escape in Settings
returns to the pause menu; Escape there resumes. Main Menu and Quit wait for the
current world save to finish. If saving fails, the pause menu stays open with
the error so it can be retried.

Settings apply immediately: render distance (2–16 chunks), mouse sensitivity,
FOV (40–110 degrees), master volume and VSync. A changed render distance is
applied when gameplay resumes; it does not require restarting the game.
Preferences live separately in `config/settings.ron`, written atomically in the
background and loaded at startup. They apply to every save slot. Audio volume
controls both new and already-playing Bevy audio; this change adds no sound assets.

## World saves

Metadata lives in `saves/<world>/world.save`. The default slot is `default-world`;
configure `PersistenceSettings` before startup to select a slot/root. Slot names
allow 1–128 ASCII letters, digits, hyphens and underscores.

The v2 file contains `JABASAVE` (8 bytes), a little-endian `u32` format version,
then a serde/Postcard payload. [Postcard's wire format](https://postcard.jamesmunns.com/wire-format)
is stable; our schema is explicitly versioned because adding/reordering fields
still requires a migration. Metadata includes the format version, world name,
`u64` seed, elapsed game days (`f64`), player position (`[f32; 3]`), and yaw/pitch
in radians. Disk types are independent of Bevy's math serialization.

The seed and clock load before scene generation. Player position and look are
restored after spawning, before the first gameplay frame; velocity starts at
zero. Metadata and player-edited chunks autosave every 60 seconds by default,
on F5/manual-save requests, and on normal application exit. Set
`autosave_interval_seconds` to another duration, or to zero to disable only the
periodic trigger. Serialization and disk I/O run on Bevy's background I/O
pool; only one save job runs at a time, with overlapping requests coalesced into
the next job. The HUD shows `Saving...` while a job is active.

Chunk overrides live beside slots as `saves/<world>/chunks.save`. Generated
terrain itself is not archived; only chunks changed by the player are kept,
including dirty chunks unloaded by streaming. Edits made while a save is running
stay dirty and are included in a follow-up save.

Writes use a unique temporary file in the same world directory, `sync_all`, and
atomic replacement of each save file; Unix also syncs the containing directory.
Leftover temporary files from an interrupted process are never loaded as saves.
Force-killing the process can lose changes since the last completed autosave.

The previous v1 text file (time only) loads automatically and is replaced with
v2 on the next successful save, using the configured seed and initial player
pose for fields that did not exist. Unknown versions, corrupt files, oversized
metadata (over 64 KiB), non-finite values, invalid rotations/coordinates, and
world-name mismatches disable writing for that session so the original file is
preserved. Startup logs the failure; restore/fix the file and restart to resume
saving. Position components must be within ±1,000,000 blocks.

Inventory is not persisted yet.

## Texture credits

The default block atlas uses selected textures adapted from the official
[Faithful 32x Java](https://github.com/Faithful-Resource-Pack/Faithful-32x-Java)
resource pack (revision `cf009450b9b868ede1c354795433c3fbdda1205d`). Grass,
foliage, and water colors are baked into the atlas because Rustcraft does not
yet implement Minecraft-style biome tinting or animated textures.

Tool and weapon icons use the unmodified 32×32 item textures from the same
official repository's `java-latest` branch at revision
`b27e488bfaf5d69230ec5922fc61315d3c8b2b64`.

Faithful 32x is copyright © Faithful Resource Pack and is used under the
[Faithful License](https://faithfulpack.net/license). JabaCraft is not an
official Faithful project and is not endorsed by the Faithful team.


## Greedy voxel meshing

Opaque coplanar faces merge into rectangles only when block ID, face texture and
constant vertex light/AO match. Nonuniform lighting stays on unit faces, preserving
the original interpolation and triangle diagonal. Leaves/cutout and water retain
unit faces. Chunk boundary culling is unchanged.

Chunk CPU meshing runs on `AsyncComputeTaskPool` with a bounded number of jobs:
`Requested -> Generating -> Generated -> Meshing -> Ready`. Workers receive an
owned 3x3x3 chunk snapshot and return mesh data; only the main world creates or
updates Bevy mesh assets. Every voxel or visible-boundary change advances a
mesh revision, so results computed from an older revision are discarded and
queued again. Generation and meshing priorities favor nearby chunks first and
then slightly favor chunks along the camera's yaw/pitch direction. Priorities
refresh when the player moves, looks far enough in a new direction, or new work
arrives, so obsolete queued work cannot accumulate after fast movement. Jobs
outside the current streaming window are cancelled.

The default scheduler budgets are four generation tasks, four meshing tasks,
two meshing task starts per frame and two completed chunk-mesh uploads per
frame. Completed worker results count toward the meshing-task limit until the
main world uploads them, bounding both CPU pressure and queued mesh memory.

Merged quads carry block-local repeat coordinates in UV1. A StandardMaterial
extension repeats the original half-texel-inset atlas tile instead of stretching
it; UV0 and all original unit faces stay unchanged. The runtime atlas gives each
Faithful tile a replicated gutter and independently generated mip levels. Linear
minification removes distant colored moire while nearest magnification keeps the
pixel-art look nearby, without bleeding adjacent materials together. PBR
lighting, fog and shadows remain enabled. Opaque atlas tiles are verified to
have full alpha, so the stock alpha prepass also remains valid.

The old mesher is retained only under `cfg(test)` as a comparison reference.
Run `cargo test greedy_benchmark -- --ignored --nocapture` for vertex/index counts
and average full-mesher time over 20 runs (including light-map construction).
These are deterministic synthetic fixtures, not an FPS benchmark.

| Fixture | Vertices old → greedy | Indices old → greedy | Mean ms old → greedy |
| --- | ---: | ---: | ---: |
| Solid chunk | 6144 → 1044 | 9216 → 1566 | 4.098 → 3.188 |
| Flat slab | 2304 → 1044 | 3456 → 1566 | 2.047 → 2.109 |
| Checkerboard | 49152 → 49152 | 73728 → 73728 | 7.586 → 7.215 |
| Mixed materials/water/leaves | 3200 → 1440 | 4800 → 2160 | 2.429 → 2.300 |
| Uneven terrain fixture | 13016 → 10628 | 19524 → 15942 | 3.403 → 3.175 |

Timing is one local optimized test-profile run and varies with hardware/load.
Geometry reduction does not guarantee faster CPU meshing for every shape; the
slab case was slightly slower in this run. Most remaining geometry on flat
surfaces preserves lighting gradients.

`cargo run --example greedy_smoke` opens a temporary world, exercises the real
GPU material/shadow pipelines and exits automatically after 180 playable frames.
Its saves and settings are isolated from normal worlds.

## How world generation works

Terrain is deterministic for `(seed, world x, world z)`. A biome sampler chooses
the surface height and surface/subsurface blocks; deeper layers are stone. Caves
are carved from continuous 3D noise, then coal and iron veins are generated from
coordinate-stable random fields. Trees and their cross-chunk canopy blocks are
placed after terrain and ores. Columns below the fixed sea level receive still
water, while caves remain dry. Chunks predicted to be above the maximum possible
terrain/tree height use an air fast path.

## Save format overview

Each world lives in `saves/<world>/`. `world.save` stores versioned serde/Postcard
metadata: world name, seed, elapsed game time, player position, and rotation.
`chunks.save` stores only player-edited chunk overrides. Saves are serialized on
the background I/O pool and written through a temporary sibling file followed by
an atomic replacement; the current save is never overwritten after a validation
failure. Autosave runs every 60 seconds by default, F5 triggers a manual save,
and normal exit waits for the final save. Preferences are separate in
`config/settings.ron`.

## Known limitations

- Inventory contents and dropped items are not persisted yet.
- Durability wear and melee combat are not implemented yet; their tool values
  are defined and validated but are not consumed by gameplay.
- Water is static: there is no fluid simulation, oxygen, or liquid spread.
- The chunk archive is a single file and can take longer to serialize as the
  number of edited chunks grows.
- Transparent water uses Bevy's normal sorting and has no order-independent
  transparency or animated surface simulation.
- Terrain generation, lighting, and mesh uploads are bounded but still depend on
  hardware and render distance; the F3 overlay is the intended diagnostic tool.
- There is no multiplayer, mod/plugin API, server mode, or combat system.
- Texture animation and biome color tinting are not implemented.

## Roadmap

The MVP is considered complete. Follow-up work should stay incremental:

1. Add compact/incremental region storage if save profiling shows the monolithic
   chunk archive becoming a real bottleneck.
2. Persist inventory and dropped-item state with an explicit save schema version.
3. Improve water presentation and optional fluid simulation without changing the
   static-world compatibility contract.
4. Add user-facing graphics/audio accessibility options and automated smoke tests
   for packaged release builds.

Large systems such as multiplayer, mod loading, and a full entity-component
voxel representation are intentionally outside this MVP.
