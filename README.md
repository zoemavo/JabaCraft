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

## Texture credits

The default block atlas uses selected textures adapted from the official
[Faithful 32x Java](https://github.com/Faithful-Resource-Pack/Faithful-32x-Java)
resource pack (revision `cf009450b9b868ede1c354795433c3fbdda1205d`). Grass,
foliage, and water colors are baked into the atlas because Rustcraft does not
yet implement Minecraft-style biome tinting or animated textures.

Faithful 32x is copyright © Faithful Resource Pack and is used under the
[Faithful License](https://faithfulpack.net/license). JabaCraft is not an
official Faithful project and is not endorsed by the Faithful team.
