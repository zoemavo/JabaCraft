use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::Mesh,
    render::render_resource::PrimitiveTopology,
};

use crate::{
    block::{BlockFace, BlockId, BlockRegistry, TextureIndex},
    chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk, ChunkStorage},
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
) -> ChunkMeshes {
    let buffers =
        build_chunk_mesh_buffers_with_seed(chunk_position, storage, registry, terrain_seed);
    ChunkMeshes {
        opaque: buffers.opaque.into_mesh(),
        water: buffers.water.into_mesh(),
    }
}

pub(super) struct ChunkMeshes {
    pub opaque: Mesh,
    pub water: Mesh,
}

#[derive(Default)]
struct SplitBuffers {
    opaque: ChunkMeshBuffers,
    water: ChunkMeshBuffers,
}

#[cfg(test)]
fn build_chunk_mesh_buffers(
    chunk_position: ChunkPos,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
) -> ChunkMeshBuffers {
    build_chunk_mesh_buffers_naive(chunk_position, storage, registry, 0).opaque
}

#[cfg(test)]
fn build_chunk_mesh_buffers_naive(
    chunk_position: ChunkPos,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
    terrain_seed: u64,
) -> SplitBuffers {
    let Some(chunk) = storage.get_chunk(chunk_position) else {
        return SplitBuffers::default();
    };
    if chunk.is_all_air() {
        return SplitBuffers::default();
    }
    let mut buffers = SplitBuffers::default();
    let mut lights = None;

    for y in 0..CHUNK_HEIGHT {
        for z in 0..CHUNK_DEPTH {
            for x in 0..CHUNK_WIDTH {
                let local = LocalBlockPos::new(x, y, z).expect("loop stays inside chunk bounds");
                let block = chunk.get_local(local);
                if block == BlockId::AIR {
                    continue;
                }

                let world = chunk_position.world_block(local);
                for face in FACES {
                    if is_local_face_visible(
                        chunk_position,
                        local,
                        block,
                        face,
                        chunk,
                        storage,
                        registry,
                    ) {
                        let texture = registry.texture_for(block, face.block_face);
                        let lights = lights.get_or_insert_with(|| {
                            ChunkLightMap::build(chunk_position, storage, registry, terrain_seed)
                        });
                        let target = if block == BlockId::WATER {
                            &mut buffers.water
                        } else {
                            &mut buffers.opaque
                        };
                        target.push_face(
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

/// Greedy rectangles are restricted to constant vertex light/AO. This conservative
/// rule preserves the old piecewise-linear lighting exactly, including diagonals.
#[derive(Clone, Copy, PartialEq)]
struct MergeKey {
    block: BlockId,
    texture: TextureIndex,
    color: [f32; 4],
}

fn build_chunk_mesh_buffers_with_seed(
    position: ChunkPos,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
    seed: u64,
) -> SplitBuffers {
    let Some(chunk) = storage.get_chunk(position) else {
        return SplitBuffers::default();
    };
    if chunk.is_all_air() {
        return SplitBuffers::default();
    }
    let mut output = SplitBuffers::default();
    let mut lightmap = None;
    let dimensions = [CHUNK_WIDTH, CHUNK_HEIGHT, CHUNK_DEPTH];
    let mut mask = [None; CHUNK_WIDTH * CHUNK_HEIGHT];
    debug_assert!(CHUNK_WIDTH * CHUNK_DEPTH <= mask.len());
    debug_assert!(CHUNK_HEIGHT * CHUNK_DEPTH <= mask.len());
    for face in FACES {
        let axis = face.normal.iter().position(|n| *n != 0.0).unwrap();
        let a = if axis == 0 { 1 } else { 0 };
        let b = if axis == 2 { 1 } else { 2 };
        let width = dimensions[a];
        let height = dimensions[b];
        for slice in 0..dimensions[axis] {
            mask[..width * height].fill(None);
            for v in 0..height {
                for u in 0..width {
                    let mut cell = [0; 3];
                    cell[axis] = slice;
                    cell[a] = u;
                    cell[b] = v;
                    let local = LocalBlockPos::new(cell[0], cell[1], cell[2]).unwrap();
                    let block = chunk.get_local(local);
                    if block == BlockId::AIR {
                        continue;
                    }
                    let world = position.world_block(local);
                    if !is_local_face_visible(
                        position, local, block, face, chunk, storage, registry,
                    ) {
                        continue;
                    }
                    let lights = lightmap.get_or_insert_with(|| {
                        ChunkLightMap::build(position, storage, registry, seed)
                    });
                    let colors = face
                        .vertices
                        .map(|vertex| vertex_light_color(world, face.block_face, vertex, lights));
                    let texture = registry.texture_for(block, face.block_face);
                    if !registry.is_transparent(block) && colors.iter().all(|c| *c == colors[0]) {
                        mask[v * width + u] = Some(MergeKey {
                            block,
                            texture,
                            color: colors[0],
                        });
                    } else {
                        let target = if block == BlockId::WATER {
                            &mut output.water
                        } else {
                            &mut output.opaque
                        };
                        target.push_quad(cell.map(|n| n as f32), face, texture, colors, [1.0; 3]);
                    }
                }
            }
            for v in 0..height {
                for u in 0..width {
                    let Some(key) = mask[v * width + u] else {
                        continue;
                    };
                    let mut w = 1;
                    while u + w < width && mask[v * width + u + w] == Some(key) {
                        w += 1;
                    }
                    let mut h = 1;
                    while v + h < height
                        && (0..w).all(|x| mask[(v + h) * width + u + x] == Some(key))
                    {
                        h += 1;
                    }
                    for y in v..v + h {
                        for x in u..u + w {
                            mask[y * width + x] = None;
                        }
                    }
                    let mut origin = [0.0; 3];
                    origin[axis] = slice as f32;
                    origin[a] = u as f32;
                    origin[b] = v as f32;
                    let mut extent = [1.0; 3];
                    extent[a] = w as f32;
                    extent[b] = h as f32;
                    output
                        .opaque
                        .push_quad(origin, face, key.texture, [key.color; 4], extent);
                }
            }
        }
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn is_local_face_visible(
    chunk_position: ChunkPos,
    local: LocalBlockPos,
    block: BlockId,
    face: Face,
    chunk: &Chunk,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
) -> bool {
    let neighbor_x = local.x() as i32 + face.neighbor[0];
    let neighbor_y = local.y() as i32 + face.neighbor[1];
    let neighbor_z = local.z() as i32 + face.neighbor[2];
    let neighbor = if (0..CHUNK_WIDTH as i32).contains(&neighbor_x)
        && (0..CHUNK_HEIGHT as i32).contains(&neighbor_y)
        && (0..CHUNK_DEPTH as i32).contains(&neighbor_z)
    {
        let local = LocalBlockPos::new(
            neighbor_x as usize,
            neighbor_y as usize,
            neighbor_z as usize,
        )
        .expect("validated neighbor is inside the current chunk");
        Some(chunk.get_local(local))
    } else {
        let world = chunk_position.world_block(local);
        let neighbor = WorldBlockPos::new(
            world.x + face.neighbor[0],
            world.y + face.neighbor[1],
            world.z + face.neighbor[2],
        );
        storage.get_block(neighbor)
    };
    neighbor.is_none_or(|other| {
        registry.is_transparent(other) && !(block == BlockId::WATER && other == BlockId::WATER)
    })
}

/// A face is visible when the adjacent world block is absent, air, or transparent.
///
/// Looking up by world position is essential: the same code handles neighbors
/// inside the current chunk and neighbors stored in an adjacent chunk.
#[cfg(test)]
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

    let current = storage.get_block(block_position);
    storage.get_block(neighbor).is_none_or(|block| {
        registry.is_transparent(block)
            && !(current == Some(BlockId::WATER) && block == BlockId::WATER)
    })
}

#[derive(Default)]
struct ChunkMeshBuffers {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    repeats: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl ChunkMeshBuffers {
    #[cfg(test)]
    fn push_face(
        &mut self,
        block: [f32; 3],
        world: WorldBlockPos,
        face: Face,
        texture: TextureIndex,
        lights: &ChunkLightMap,
    ) {
        let colors = face
            .vertices
            .map(|vertex| vertex_light_color(world, face.block_face, vertex, lights));
        self.push_quad(block, face, texture, colors, [1.0; 3]);
    }

    fn push_quad(
        &mut self,
        block: [f32; 3],
        face: Face,
        texture: TextureIndex,
        colors: [[f32; 4]; 4],
        extent: [f32; 3],
    ) {
        let first = self.positions.len() as u32;
        let uvs = face_uvs(texture, face.block_face);
        let unit = match face.block_face {
            BlockFace::North | BlockFace::South => [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            _ => [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        };
        let repeats = match face.block_face {
            BlockFace::East | BlockFace::West => [extent[2], extent[1]],
            BlockFace::Top | BlockFace::Bottom => [extent[0], extent[2]],
            _ => [extent[0], extent[1]],
        };
        for i in 0..4 {
            self.positions.push(std::array::from_fn(|axis| {
                block[axis] + face.vertices[i][axis] * extent[axis]
            }));
            self.normals.push(face.normal);
            self.uvs.push(uvs[i]);
            self.colors.push(colors[i]);
            self.repeats.push(if repeats == [1.0; 2] {
                [-1.0; 2]
            } else {
                [unit[i][0] * repeats[0], unit[i][1] * repeats[1]]
            });
        }
        self.indices
            .extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
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
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.repeats)
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
    fn water_is_separate_and_hides_internal_faces_on_every_axis() {
        let position = ChunkPos::new(0, 0, 0);
        for offset in [[1, 0, 0], [0, 1, 0], [0, 0, 1]] {
            let mut storage = storage_with_chunk(position);
            storage
                .set_block(WorldBlockPos::new(2, 2, 2), BlockId::WATER)
                .unwrap();
            storage
                .set_block(
                    WorldBlockPos::new(2 + offset[0], 2 + offset[1], 2 + offset[2]),
                    BlockId::WATER,
                )
                .unwrap();
            let buffers = build_chunk_mesh_buffers_with_seed(
                position,
                &storage,
                &BlockRegistry::default(),
                0,
            );
            assert_eq!(buffers.opaque.positions.len(), 0);
            assert_eq!(buffers.water.positions.len(), 10 * 4);
            assert_eq!(buffers.water.indices.len(), 10 * 6);
        }
    }

    #[test]
    fn water_culls_across_negative_and_vertical_chunk_boundaries() {
        for (a, b) in [
            (WorldBlockPos::new(-1, 2, 2), WorldBlockPos::new(0, 2, 2)),
            (WorldBlockPos::new(2, -1, 2), WorldBlockPos::new(2, 0, 2)),
            (WorldBlockPos::new(2, 2, -1), WorldBlockPos::new(2, 2, 0)),
        ] {
            let mut storage = storage_with_chunk(a.split().0);
            storage.set_block(a, BlockId::WATER).unwrap();
            let exposed = build_chunk_mesh_buffers_with_seed(
                a.split().0,
                &storage,
                &BlockRegistry::default(),
                0,
            );
            assert_eq!(exposed.water.positions.len(), 6 * 4);
            storage.insert_chunk(b.split().0, Chunk::default());
            storage.set_block(b, BlockId::WATER).unwrap();
            for position in [a.split().0, b.split().0] {
                let buffers = build_chunk_mesh_buffers_with_seed(
                    position,
                    &storage,
                    &BlockRegistry::default(),
                    0,
                );
                assert_eq!(buffers.water.positions.len(), 5 * 4);
            }
        }
    }

    #[test]
    fn solid_shore_face_remains_visible_through_water() {
        let position = ChunkPos::new(0, 0, 0);
        let mut storage = storage_with_chunk(position);
        storage
            .set_block(WorldBlockPos::new(2, 2, 2), BlockId::STONE)
            .unwrap();
        storage
            .set_block(WorldBlockPos::new(3, 2, 2), BlockId::WATER)
            .unwrap();
        let buffers =
            build_chunk_mesh_buffers_with_seed(position, &storage, &BlockRegistry::default(), 0);
        assert_eq!(buffers.opaque.positions.len(), 6 * 4);
        assert_eq!(buffers.water.positions.len(), 5 * 4);
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
        assert_eq!(top[0], [80.5 / 256.0, 47.5 / 256.0]);
        assert_eq!(bottom[0], [144.5 / 256.0, 47.5 / 256.0]);
        assert_eq!(side[0], [208.5 / 256.0, 47.5 / 256.0]);
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

#[cfg(test)]
#[path = "greedy_tests.rs"]
mod greedy_tests;
