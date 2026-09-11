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

## Texture credits

The default block atlas uses selected textures adapted from the official
[Faithful 32x Java](https://github.com/Faithful-Resource-Pack/Faithful-32x-Java)
resource pack (revision `cf009450b9b868ede1c354795433c3fbdda1205d`). Grass,
foliage, and water colors are baked into the atlas because Rustcraft does not
yet implement Minecraft-style biome tinting or animated textures.

Faithful 32x is copyright © Faithful Resource Pack and is used under the
[Faithful License](https://faithfulpack.net/license). JabaCraft is not an
official Faithful project and is not endorsed by the Faithful team.
