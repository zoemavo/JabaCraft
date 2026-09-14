use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

pub(super) type VoxelMaterial = ExtendedMaterial<StandardMaterial, AtlasRepeat>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub(super) struct AtlasRepeat {}

impl MaterialExtension for AtlasRepeat {
    fn fragment_shader() -> ShaderRef {
        "shaders/greedy_atlas.wgsl".into()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/greedy_atlas.wgsl".into()
    }
    // The default alpha prepass is exact: merged faces are strictly opaque,
    // while cutout faces retain their original unit atlas UVs.
}
