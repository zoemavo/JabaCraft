use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::Mesh,
    render::render_resource::PrimitiveTopology,
};

use crate::{
    block::{BlockFace, BlockRegistry, TextureIndex},
    chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, ChunkStorage},
    coordinates::{ChunkPos, LocalBlockPos, WorldBlockPos},
};

use super::{
    atlas::atlas_uvs,
    lighting::{ChunkLightMap, vertex_light_color},
};

pub(super) fn build_chunk_mesh(
    chunk_position: ChunkPos,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
    terrain_seed: u64,
) -> Mesh {
    build_chunk_mesh_buffers_with_seed(chunk_position, storage, registry, terrain_seed).into_mesh()
}

#[cfg(test)]
fn build_chunk_mesh_buffers(
    chunk_position: ChunkPos,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
) -> ChunkMeshBuffers {
    build_chunk_mesh_buffers_with_seed(chunk_position, storage, registry, 0)
}

fn build_chunk_mesh_buffers_with_seed(
    chunk_position: ChunkPos,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
    terrain_seed: u64,
) -> ChunkMeshBuffers {
    let Some(chunk) = storage.get_chunk(chunk_position) else {
        return ChunkMeshBuffers::default();
    };
    if chunk.is_all_air() {
        return ChunkMeshBuffers::default();
    }
    let mut buffers = ChunkMeshBuffers::default();
    let mut lights = None;

    for y in 0..CHUNK_HEIGHT {
        for z in 0..CHUNK_DEPTH {
            for x in 0..CHUNK_WIDTH {
                let local = LocalBlockPos::new(x, y, z).expect("loop stays inside chunk bounds");
                let block = chunk.get_local(local);
                if !registry.is_solid(block) {
                    continue;
                }

                let world = chunk_position.world_block(local);
                for face in FACES {
                    if is_face_visible(world, face, storage, registry) {
                        let texture = registry.texture_for(block, face.block_face);
                        let lights = lights.get_or_insert_with(|| {
                            ChunkLightMap::build(chunk_position, storage, registry, terrain_seed)
                        });
                        buffers.push_face(
                            [x as f32, y as f32, z as f32],
                            world,
                            face,
                            texture,
                            lights,
                        );
                    }
                }
            }
        }
    }

    buffers
}

/// A face is visible when the adjacent world block is absent, air, or transparent.
///
/// Looking up by world position is essential: the same code handles neighbors
/// inside the current chunk and neighbors stored in an adjacent chunk.
fn is_face_visible(
    block_position: WorldBlockPos,
    face: Face,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
) -> bool {
    let neighbor = WorldBlockPos::new(
        block_position.x + face.neighbor[0],
        block_position.y + face.neighbor[1],
        block_position.z + face.neighbor[2],
    );

    storage
        .get_block(neighbor)
        .is_none_or(|block| registry.is_transparent(block))
}

#[derive(Default)]
struct ChunkMeshBuffers {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl ChunkMeshBuffers {
    fn push_face(
        &mut self,
        block: [f32; 3],
        world: WorldBlockPos,
        face: Face,
        texture: TextureIndex,
        lights: &ChunkLightMap,
    ) {
        let first_vertex = self.positions.len() as u32;
        let uvs = face_uvs(texture, face.block_face);

        for (vertex, uv) in face.vertices.into_iter().zip(uvs) {
            self.positions.push([
                block[0] + vertex[0],
                block[1] + vertex[1],
                block[2] + vertex[2],
            ]);
            self.normals.push(face.normal);
            self.uvs.push(uv);
            self.colors
                .push(vertex_light_color(world, face.block_face, vertex, lights));
        }
        self.indices.extend_from_slice(&[
            first_vertex,
            first_vertex + 1,
            first_vertex + 2,
            first_vertex,
            first_vertex + 2,
            first_vertex + 3,
        ]);
    }

    fn into_mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

fn face_uvs(texture: TextureIndex, face: BlockFace) -> [[f32; 2]; 4] {
    let uvs = atlas_uvs(texture);
    match face {
        // North and south use horizontal-first vertex winding to keep their
        // normals facing outward. Reorder the UV corners so texture "up"
        // still points toward world +Y instead of toward the side.
        BlockFace::North | BlockFace::South => [uvs[0], uvs[3], uvs[2], uvs[1]],
        _ => uvs,
    }
}

#[derive(Clone, Copy)]
struct Face {
    block_face: BlockFace,
    neighbor: [i32; 3],
    normal: [f32; 3],
    vertices: [[f32; 3]; 4],
}

const EAST: Face = Face {
    block_face: BlockFace::East,
    neighbor: [1, 0, 0],
    normal: [1.0, 0.0, 0.0],
    vertices: [
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
    ],
};

const WEST: Face = Face {
    block_face: BlockFace::West,
    neighbor: [-1, 0, 0],
    normal: [-1.0, 0.0, 0.0],
    vertices: [
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
    ],
};

const TOP: Face = Face {
    block_face: BlockFace::Top,
    neighbor: [0, 1, 0],
    normal: [0.0, 1.0, 0.0],
    vertices: [
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
    ],
};

const BOTTOM: Face = Face {
    block_face: BlockFace::Bottom,
    neighbor: [0, -1, 0],
    normal: [0.0, -1.0, 0.0],
    vertices: [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
    ],
};

const SOUTH: Face = Face {
    block_face: BlockFace::South,
    neighbor: [0, 0, 1],
    normal: [0.0, 0.0, 1.0],
    vertices: [
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ],
};

const NORTH: Face = Face {
    block_face: BlockFace::North,
    neighbor: [0, 0, -1],
    normal: [0.0, 0.0, -1.0],
    vertices: [
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ],
};

const FACES: [Face; 6] = [EAST, WEST, TOP, BOTTOM, SOUTH, NORTH];

#[cfg(test)]
mod tests {
    use crate::{block::BlockId, chunk::Chunk};

    use super::*;

    fn storage_with_chunk(position: ChunkPos) -> ChunkStorage {
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(position, Chunk::default());
        storage
    }

    #[test]
    fn face_next_to_air_is_visible() {
        let chunk_position = ChunkPos::new(0, 0, 0);
        let mut storage = storage_with_chunk(chunk_position);
        let block_position = WorldBlockPos::new(1, 1, 1);
        storage.set_block(block_position, BlockId::STONE).unwrap();

        assert!(is_face_visible(
            block_position,
            EAST,
            &storage,
            &BlockRegistry::default()
        ));
    }

    #[test]
    fn face_next_to_transparent_block_is_visible() {
        let chunk_position = ChunkPos::new(0, 0, 0);
        let mut storage = storage_with_chunk(chunk_position);
        let block_position = WorldBlockPos::new(1, 1, 1);
        storage.set_block(block_position, BlockId::STONE).unwrap();
        storage
            .set_block(WorldBlockPos::new(2, 1, 1), BlockId::LEAVES)
            .unwrap();

        assert!(is_face_visible(
            block_position,
            EAST,
            &storage,
            &BlockRegistry::default()
        ));
    }

    #[test]
    fn face_between_opaque_solid_blocks_is_hidden() {
        let chunk_position = ChunkPos::new(0, 0, 0);
        let mut storage = storage_with_chunk(chunk_position);
        let block_position = WorldBlockPos::new(1, 1, 1);
        storage.set_block(block_position, BlockId::STONE).unwrap();
        storage
            .set_block(WorldBlockPos::new(2, 1, 1), BlockId::GRASS)
            .unwrap();

        assert!(!is_face_visible(
            block_position,
            EAST,
            &storage,
            &BlockRegistry::default()
        ));
    }

    #[test]
    fn face_between_positive_neighbor_chunks_is_hidden() {
        let left_position = ChunkPos::new(0, 0, 0);
        let right_position = ChunkPos::new(1, 0, 0);
        let mut storage = storage_with_chunk(left_position);
        storage.insert_chunk(right_position, Chunk::default());
        let block_position = WorldBlockPos::new(15, 1, 1);
        storage.set_block(block_position, BlockId::STONE).unwrap();
        storage
            .set_block(WorldBlockPos::new(16, 1, 1), BlockId::STONE)
            .unwrap();

        assert!(!is_face_visible(
            block_position,
            EAST,
            &storage,
            &BlockRegistry::default()
        ));
    }

    #[test]
    fn face_between_negative_neighbor_chunks_is_hidden() {
        let center_position = ChunkPos::new(0, 0, 0);
        let negative_position = ChunkPos::new(-1, 0, 0);
        let mut storage = storage_with_chunk(center_position);
        storage.insert_chunk(negative_position, Chunk::default());
        let block_position = WorldBlockPos::new(0, 1, 1);
        storage.set_block(block_position, BlockId::STONE).unwrap();
        storage
            .set_block(WorldBlockPos::new(-1, 1, 1), BlockId::STONE)
            .unwrap();

        assert!(!is_face_visible(
            block_position,
            WEST,
            &storage,
            &BlockRegistry::default()
        ));
    }

    #[test]
    fn face_between_vertical_neighbor_chunks_is_hidden() {
        let lower_position = ChunkPos::new(0, -2, 0);
        let upper_position = ChunkPos::new(0, -1, 0);
        let mut storage = storage_with_chunk(lower_position);
        storage.insert_chunk(upper_position, Chunk::default());
        let lower_block = WorldBlockPos::new(1, -17, 1);
        storage.set_block(lower_block, BlockId::STONE).unwrap();
        storage
            .set_block(WorldBlockPos::new(1, -16, 1), BlockId::STONE)
            .unwrap();

        assert!(!is_face_visible(
            lower_block,
            TOP,
            &storage,
            &BlockRegistry::default()
        ));
    }

    #[test]
    fn face_at_unloaded_chunk_boundary_is_visible() {
        let chunk_position = ChunkPos::new(0, 0, 0);
        let mut storage = storage_with_chunk(chunk_position);
        let block_position = WorldBlockPos::new(15, 1, 1);
        storage.set_block(block_position, BlockId::STONE).unwrap();

        assert!(is_face_visible(
            block_position,
            EAST,
            &storage,
            &BlockRegistry::default()
        ));
    }

    #[test]
    fn isolated_voxel_emits_six_indexed_faces_with_all_attributes() {
        let position = ChunkPos::new(0, 0, 0);
        let mut storage = storage_with_chunk(position);
        storage
            .set_block(WorldBlockPos::new(1, 1, 1), BlockId::STONE)
            .unwrap();

        let buffers = build_chunk_mesh_buffers(position, &storage, &BlockRegistry::default());

        assert_eq!(buffers.positions.len(), 6 * 4);
        assert_eq!(buffers.normals.len(), 6 * 4);
        assert_eq!(buffers.uvs.len(), 6 * 4);
        assert_eq!(buffers.colors.len(), 6 * 4);
        assert_eq!(buffers.indices.len(), 6 * 6);

        let mesh = buffers.into_mesh();
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
        assert!(mesh.indices().is_some());
    }

    #[test]
    fn two_adjacent_voxels_do_not_emit_their_internal_faces() {
        let position = ChunkPos::new(0, 0, 0);
        let mut storage = storage_with_chunk(position);
        storage
            .set_block(WorldBlockPos::new(1, 1, 1), BlockId::STONE)
            .unwrap();
        storage
            .set_block(WorldBlockPos::new(2, 1, 1), BlockId::STONE)
            .unwrap();

        let buffers = build_chunk_mesh_buffers(position, &storage, &BlockRegistry::default());

        assert_eq!(buffers.positions.len(), 10 * 4);
        assert_eq!(buffers.indices.len(), 10 * 6);
    }

    #[test]
    fn adjacent_voxels_across_chunk_boundary_hide_shared_face_in_mesh() {
        let left_position = ChunkPos::new(0, 0, 0);
        let right_position = ChunkPos::new(1, 0, 0);
        let mut storage = storage_with_chunk(left_position);
        storage.insert_chunk(right_position, Chunk::default());
        storage
            .set_block(WorldBlockPos::new(15, 1, 1), BlockId::STONE)
            .unwrap();
        storage
            .set_block(WorldBlockPos::new(16, 1, 1), BlockId::STONE)
            .unwrap();

        let buffers = build_chunk_mesh_buffers(left_position, &storage, &BlockRegistry::default());

        assert_eq!(buffers.positions.len(), 5 * 4);
        assert_eq!(buffers.indices.len(), 5 * 6);
    }

    #[test]
    fn grass_faces_use_different_atlas_cells() {
        let registry = BlockRegistry::default();
        let top = atlas_uvs(registry.texture_for(BlockId::GRASS, BlockFace::Top));
        let bottom = atlas_uvs(registry.texture_for(BlockId::GRASS, BlockFace::Bottom));
        let side = atlas_uvs(registry.texture_for(BlockId::GRASS, BlockFace::North));

        assert_ne!(top, bottom);
        assert_ne!(top, side);
        assert_ne!(bottom, side);
        assert_eq!(top[0], [32.5 / 128.0, 31.5 / 128.0]);
        assert_eq!(bottom[0], [64.5 / 128.0, 31.5 / 128.0]);
        assert_eq!(side[0], [96.5 / 128.0, 31.5 / 128.0]);
    }

    #[test]
    fn side_texture_top_always_points_toward_world_up() {
        for face in [EAST, WEST, SOUTH, NORTH] {
            let uvs = face_uvs(3, face.block_face);
            let top_v = uvs.iter().map(|uv| uv[1]).fold(f32::INFINITY, f32::min);
            let bottom_v = uvs.iter().map(|uv| uv[1]).fold(f32::NEG_INFINITY, f32::max);

            for (vertex, uv) in face.vertices.into_iter().zip(uvs) {
                let expected_v = if vertex[1] == 1.0 { top_v } else { bottom_v };
                assert_eq!(
                    uv[1], expected_v,
                    "{:?} texture is rotated",
                    face.block_face
                );
            }
        }
    }
}
