//! Deterministic biome terrain and simple cross-chunk tree generation.

use crate::{
    block::BlockId,
    chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk},
    coordinates::{ChunkPos, LocalBlockPos},
};

use super::{
    BiomeSampler, CaveSettings,
    cave::carve_caves,
    features::{MAX_TREE_BLOCKS_ABOVE_SURFACE, generate_features_for_chunk},
    ore::generate_ores_for_chunk,
};

const SUBSURFACE_DEPTH: i32 = 3;
// These are conservative bounds derived from the current biome definitions.
const MIN_TERRAIN_HEIGHT: i32 = -2;
const MAX_TERRAIN_HEIGHT: i32 = 26;
const MAX_GENERATED_BLOCK_Y: i32 = MAX_TERRAIN_HEIGHT + MAX_TREE_BLOCKS_ABOVE_SURFACE;

pub(super) fn generate_terrain_chunk(
    position: ChunkPos,
    seed: u64,
    cave_settings: CaveSettings,
) -> Chunk {
    if is_chunk_potentially_empty(position) {
        return Chunk::new(BlockId::AIR);
    }

    let sampler = BiomeSampler::new(seed);
    let chunk_min_y = i64::from(position.y) * CHUNK_HEIGHT as i64;
    let chunk_max_y = chunk_min_y + CHUNK_HEIGHT as i64 - 1;
    if chunk_max_y < i64::from(MIN_TERRAIN_HEIGHT - SUBSURFACE_DEPTH) {
        let mut chunk = Chunk::new(BlockId::STONE);
        carve_caves(&mut chunk, position, seed, sampler, cave_settings);
        generate_ores_for_chunk(&mut chunk, position, seed, sampler);
        return chunk;
    }

    let chunk_min_x = i64::from(position.x) * CHUNK_WIDTH as i64;
    let chunk_min_z = i64::from(position.z) * CHUNK_DEPTH as i64;
    let mut chunk = Chunk::new(BlockId::AIR);

    for local_z in 0..CHUNK_DEPTH {
        for local_x in 0..CHUNK_WIDTH {
            let world_x = chunk_min_x + local_x as i64;
            let world_z = chunk_min_z + local_z as i64;
            let sample = sampler.sample(world_x, world_z);
            let definition = sample.biome.definition();

            for local_y in 0..CHUNK_HEIGHT {
                let world_y = chunk_min_y + local_y as i64;
                let block = terrain_block(
                    world_y,
                    sample.terrain_height,
                    definition.surface_block,
                    definition.subsurface_block,
                );
                if block != BlockId::AIR {
                    let local = LocalBlockPos::new(local_x, local_y, local_z)
                        .expect("generation loops stay inside chunk bounds");
                    chunk.set_local(local, block);
                }
            }
        }
    }

    // Caves precede ores so a vein can line a cave but never be erased by it.
    carve_caves(&mut chunk, position, seed, sampler, cave_settings);
    generate_ores_for_chunk(&mut chunk, position, seed, sampler);
    generate_features_for_chunk(&mut chunk, position, seed, sampler);
    chunk
}

fn terrain_block(
    world_y: i64,
    surface_y: i32,
    surface_block: BlockId,
    subsurface_block: BlockId,
) -> BlockId {
    let surface_y = i64::from(surface_y);
    if world_y > surface_y {
        BlockId::AIR
    } else if world_y == surface_y {
        surface_block
    } else if world_y >= surface_y - i64::from(SUBSURFACE_DEPTH) {
        subsurface_block
    } else {
        BlockId::STONE
    }
}

/// Conservatively predicts chunks that cannot contain terrain or tree voxels.
/// Actual voxel contents remain authoritative for rendering.
pub fn is_chunk_potentially_empty(position: ChunkPos) -> bool {
    let chunk_min_y = i64::from(position.y) * CHUNK_HEIGHT as i64;
    chunk_min_y > i64::from(MAX_GENERATED_BLOCK_Y)
}

pub fn terrain_height_at(seed: u64, world_x: i64, world_z: i64) -> i32 {
    BiomeSampler::new(seed)
        .sample(world_x, world_z)
        .terrain_height
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated_chunk(position: ChunkPos, seed: u64) -> Chunk {
        generate_terrain_chunk(position, seed, CaveSettings::default())
    }

    #[test]
    fn generation_is_deterministic() {
        let position = ChunkPos::new(-3, 0, 7);
        let first = generated_chunk(position, 123_456);
        let second = generated_chunk(position, 123_456);

        assert_eq!(first.blocks(), second.blocks());
    }

    #[test]
    fn deep_terrain_fast_path_still_generates_ore() {
        let seed = 42;
        let mut ore_count = 0;

        for z in -3..=3 {
            for x in -3..=3 {
                let chunk = generated_chunk(ChunkPos::new(x, -2, z), seed);
                ore_count += chunk
                    .blocks()
                    .iter()
                    .filter(|&&block| matches!(block, BlockId::COAL_ORE | BlockId::IRON_ORE))
                    .count();
            }
        }

        assert!(ore_count > 0);
    }

    #[test]
    fn surface_blocks_follow_the_sampled_biome() {
        let seed = 81;
        for world_x in -64_i64..=64 {
            let world_z = 17_i64;
            let sample = BiomeSampler::new(seed).sample(world_x, world_z);
            let chunk_position = ChunkPos::new(
                world_x.div_euclid(CHUNK_WIDTH as i64) as i32,
                sample.terrain_height.div_euclid(CHUNK_HEIGHT as i32),
                world_z.div_euclid(CHUNK_DEPTH as i64) as i32,
            );
            let chunk = generated_chunk(chunk_position, seed);
            let local_x = world_x.rem_euclid(CHUNK_WIDTH as i64) as usize;
            let local_y = sample.terrain_height.rem_euclid(CHUNK_HEIGHT as i32) as usize;
            let local_z = world_z.rem_euclid(CHUNK_DEPTH as i64) as usize;

            assert_eq!(
                chunk.get_block(local_x, local_y, local_z),
                Some(sample.biome.definition().surface_block)
            );
        }
    }

    #[test]
    fn tall_terrain_crosses_vertical_chunk_boundaries() {
        let seed = 42;
        let sampler = BiomeSampler::new(seed);
        let (world_x, world_z, surface_y) = (-512_i64..=512)
            .flat_map(|x| (-512_i64..=512).step_by(16).map(move |z| (x, z)))
            .find_map(|(x, z)| {
                let height = sampler.sample(x, z).terrain_height;
                (height >= CHUNK_HEIGHT as i32).then_some((x, z, height))
            })
            .expect("rocky terrain should cross y=16");
        let chunk_x = world_x.div_euclid(CHUNK_WIDTH as i64) as i32;
        let chunk_z = world_z.div_euclid(CHUNK_DEPTH as i64) as i32;
        let lower = generated_chunk(ChunkPos::new(chunk_x, 0, chunk_z), seed);
        let upper = generated_chunk(ChunkPos::new(chunk_x, 1, chunk_z), seed);
        let local_x = world_x.rem_euclid(CHUNK_WIDTH as i64) as usize;
        let local_z = world_z.rem_euclid(CHUNK_DEPTH as i64) as usize;

        assert_ne!(lower.get_block(local_x, 15, local_z), Some(BlockId::AIR));
        assert_ne!(upper.get_block(local_x, 0, local_z), Some(BlockId::AIR));
        assert!(surface_y >= 16);
    }

    #[test]
    fn empty_prediction_includes_the_highest_possible_tree() {
        assert!(!is_chunk_potentially_empty(ChunkPos::new(0, 2, 0)));
        assert!(is_chunk_potentially_empty(ChunkPos::new(0, 3, 0)));
        assert!(generated_chunk(ChunkPos::new(0, 3, 0), 42).is_all_air());
    }

    #[test]
    fn generated_deep_terrain_contains_caves_and_solid_rock() {
        let seed = 42;
        let mut air_count = 0;
        let mut stone_count = 0;

        for z in -3..=3 {
            for x in -3..=3 {
                let chunk = generated_chunk(ChunkPos::new(x, -2, z), seed);
                air_count += chunk
                    .blocks()
                    .iter()
                    .filter(|&&block| block == BlockId::AIR)
                    .count();
                stone_count += chunk
                    .blocks()
                    .iter()
                    .filter(|&&block| block == BlockId::STONE)
                    .count();
            }
        }

        assert!(
            air_count > 0,
            "default cave settings should carve underground air"
        );
        assert!(stone_count > 0, "caves must not erase all underground rock");
    }
}
