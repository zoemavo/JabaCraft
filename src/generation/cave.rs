//! Simple deterministic caves carved from continuous world-space 3D noise.

use bevy::prelude::Resource;

use crate::{
    block::BlockId,
    chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk},
    coordinates::{ChunkPos, LocalBlockPos},
};

use super::{BiomeSampler, noise::NoiseMap};

const CAVE_NOISE_SALT: u64 = 0xCA6E_51D3_8B72_04AF;
const CAVE_NOISE_OCTAVES: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub struct CaveSettings {
    /// Base 3D noise frequency in world-block coordinates.
    pub frequency: f64,
    /// Blocks with noise above this value are carved. Higher means fewer caves.
    pub threshold: f32,
    /// Number of blocks preserved below the sampled terrain surface.
    pub surface_clearance: i32,
}

impl Default for CaveSettings {
    fn default() -> Self {
        Self {
            frequency: 1.0 / 28.0,
            threshold: 0.46,
            surface_clearance: 3,
        }
    }
}

pub(super) fn carve_caves(
    chunk: &mut Chunk,
    position: ChunkPos,
    seed: u64,
    sampler: BiomeSampler,
    settings: CaveSettings,
) {
    let Some(noise) = cave_noise(seed, settings) else {
        return;
    };
    let chunk_min_x = i64::from(position.x) * CHUNK_WIDTH as i64;
    let chunk_min_y = i64::from(position.y) * CHUNK_HEIGHT as i64;
    let chunk_min_z = i64::from(position.z) * CHUNK_DEPTH as i64;

    for local_y in 0..CHUNK_HEIGHT {
        let world_y = chunk_min_y + local_y as i64;
        for local_z in 0..CHUNK_DEPTH {
            let world_z = chunk_min_z + local_z as i64;
            for local_x in 0..CHUNK_WIDTH {
                let local = LocalBlockPos::new(local_x, local_y, local_z)
                    .expect("cave loops stay inside chunk bounds");
                if !matches!(chunk.get_local(local), BlockId::STONE | BlockId::DIRT) {
                    continue;
                }

                let world_x = chunk_min_x + local_x as i64;
                let surface_y = sampler.sample(world_x, world_z).terrain_height;
                if should_carve(noise, settings, world_x, world_y, world_z, surface_y) {
                    chunk.set_local(local, BlockId::AIR);
                }
            }
        }
    }
}

fn cave_noise(seed: u64, settings: CaveSettings) -> Option<NoiseMap> {
    if !settings.frequency.is_finite() || settings.frequency <= 0.0 {
        return None;
    }
    Some(NoiseMap::new(
        seed ^ CAVE_NOISE_SALT,
        settings.frequency,
        CAVE_NOISE_OCTAVES,
    ))
}

fn should_carve(
    noise: NoiseMap,
    settings: CaveSettings,
    world_x: i64,
    world_y: i64,
    world_z: i64,
    surface_y: i32,
) -> bool {
    let protected_surface_y = i64::from(surface_y - settings.surface_clearance.max(1));
    world_y <= protected_surface_y
        && noise.sample_3d(world_x, world_y, world_z) > settings.threshold.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carved_chunk(position: ChunkPos, seed: u64, settings: CaveSettings) -> Chunk {
        let sampler = BiomeSampler::new(seed);
        let mut chunk = Chunk::new(BlockId::STONE);
        carve_caves(&mut chunk, position, seed, sampler, settings);
        chunk
    }

    #[test]
    fn cave_carving_is_deterministic() {
        let position = ChunkPos::new(-3, -2, 8);
        let settings = CaveSettings::default();

        assert_eq!(
            carved_chunk(position, 42, settings).blocks(),
            carved_chunk(position, 42, settings).blocks()
        );
    }

    #[test]
    fn caves_never_replace_non_terrain_blocks() {
        let position = ChunkPos::new(0, -2, 0);
        for fill in [
            BlockId::AIR,
            BlockId::GRASS,
            BlockId::SAND,
            BlockId::WOOD,
            BlockId::COAL_ORE,
            BlockId::IRON_ORE,
        ] {
            let mut chunk = Chunk::new(fill);
            carve_caves(
                &mut chunk,
                position,
                42,
                BiomeSampler::new(42),
                CaveSettings {
                    threshold: -1.0,
                    ..Default::default()
                },
            );
            assert!(chunk.blocks().iter().all(|&block| block == fill));
        }
    }

    #[test]
    fn protected_surface_layers_are_not_destroyed() {
        let seed = 7;
        let sampler = BiomeSampler::new(seed);
        let settings = CaveSettings {
            threshold: -1.0,
            ..Default::default()
        };
        let position = ChunkPos::new(0, 0, 0);
        let chunk = carved_chunk(position, seed, settings);

        for local_z in 0..CHUNK_DEPTH {
            for local_x in 0..CHUNK_WIDTH {
                let surface_y = sampler
                    .sample(local_x as i64, local_z as i64)
                    .terrain_height;
                for offset in 0..settings.surface_clearance {
                    let world_y = surface_y - offset;
                    if (0..CHUNK_HEIGHT as i32).contains(&world_y) {
                        assert_eq!(
                            chunk.get_block(local_x, world_y as usize, local_z),
                            Some(BlockId::STONE)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn threshold_and_frequency_change_the_carved_pattern() {
        let position = ChunkPos::new(2, -2, -4);
        let none = carved_chunk(
            position,
            99,
            CaveSettings {
                threshold: 1.0,
                ..Default::default()
            },
        );
        let many = carved_chunk(
            position,
            99,
            CaveSettings {
                threshold: -1.0,
                ..Default::default()
            },
        );
        let different_frequency = carved_chunk(
            position,
            99,
            CaveSettings {
                frequency: 1.0 / 11.0,
                threshold: 0.0,
                ..Default::default()
            },
        );
        let default_frequency = carved_chunk(
            position,
            99,
            CaveSettings {
                threshold: 0.0,
                ..Default::default()
            },
        );

        assert!(!none.blocks().contains(&BlockId::AIR));
        assert!(many.blocks().contains(&BlockId::AIR));
        assert_ne!(different_frequency.blocks(), default_frequency.blocks());
    }

    #[test]
    fn one_cave_region_crosses_a_chunk_boundary() {
        let seed = 42;
        let settings = CaveSettings::default();
        let noise = cave_noise(seed, settings).unwrap();
        let sampler = BiomeSampler::new(seed);
        let boundary_x = CHUNK_WIDTH as i64;
        let (world_y, world_z) = (-48_i64..=-8)
            .flat_map(|y| (-512_i64..=512).map(move |z| (y, z)))
            .find(|&(y, z)| {
                let left_surface = sampler.sample(boundary_x - 1, z).terrain_height;
                let right_surface = sampler.sample(boundary_x, z).terrain_height;
                should_carve(noise, settings, boundary_x - 1, y, z, left_surface)
                    && should_carve(noise, settings, boundary_x, y, z, right_surface)
            })
            .expect("continuous cave noise should cross the searched chunk edge");
        let left_position = ChunkPos::new(
            0,
            world_y.div_euclid(CHUNK_HEIGHT as i64) as i32,
            world_z.div_euclid(CHUNK_DEPTH as i64) as i32,
        );
        let right_position = ChunkPos::new(1, left_position.y, left_position.z);
        let left = carved_chunk(left_position, seed, settings);
        let right = carved_chunk(right_position, seed, settings);

        assert_eq!(
            left.get_block(
                CHUNK_WIDTH - 1,
                world_y.rem_euclid(CHUNK_HEIGHT as i64) as usize,
                world_z.rem_euclid(CHUNK_DEPTH as i64) as usize,
            ),
            Some(BlockId::AIR)
        );
        assert_eq!(
            right.get_block(
                0,
                world_y.rem_euclid(CHUNK_HEIGHT as i64) as usize,
                world_z.rem_euclid(CHUNK_DEPTH as i64) as usize,
            ),
            Some(BlockId::AIR)
        );
    }
}
