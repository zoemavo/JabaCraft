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

Each chunk has independent opaque/cutout and water mesh assets and entities.
Leaves retain the cutout material; water uses a separate matte alpha-blended,
double-sided material and does not cast shadows. Shared water faces are culled
across all chunk boundaries, while solid shore faces remain visible through
the water. Both layers are rebuilt and released with the chunk lifecycle.
Transparency uses Bevy's normal mesh sorting, not per-triangle sorting or OIT.

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
FOV (40–110 degrees), master volume and VSync. Chunk streaming continues while
paused so changing render distance loads/unloads chunks without restarting.
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
queued again. Pending work is prioritized around the player's current chunk.

Merged quads carry block-local repeat coordinates in UV1. A StandardMaterial
extension repeats the original half-texel-inset atlas tile instead of stretching
it; UV0 and all original unit faces stay unchanged. PBR lighting, fog and shadows
remain enabled. Opaque atlas tiles are verified to have full alpha, so the stock
alpha prepass also remains valid.

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
