//! Deterministic underground ore veins that can cross chunk boundaries.

use crate::{
    block::BlockId,
    chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk},
    coordinates::{ChunkPos, LocalBlockPos},
};

use super::{
    BiomeSampler,
    random::{hash_3d, unit_f32},
};

const ORE_SURFACE_CLEARANCE: i32 = 4;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum OreKind {
    Iron,
    Coal,
}

impl OreKind {
    // Iron is painted first so the rarer feature wins deterministic overlaps.
    const ALL: [Self; 2] = [Self::Iron, Self::Coal];

    const fn definition(self) -> OreDefinition {
        match self {
            Self::Coal => OreDefinition {
                block: BlockId::COAL_ORE,
                min_y: -48,
                max_y: 20,
                cell_size: 8,
                spawn_chance: 0.18,
                min_radius: 1.4,
                max_radius: 3.2,
                salt: 0xC04A_17E5_928B_63DF,
            },
            Self::Iron => OreDefinition {
                block: BlockId::IRON_ORE,
                min_y: -64,
                max_y: 2,
                cell_size: 10,
                spawn_chance: 0.065,
                min_radius: 1.2,
                max_radius: 2.6,
                salt: 0x1A07_6D3B_EC54_92F8,
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct OreDefinition {
    block: BlockId,
    min_y: i32,
    max_y: i32,
    cell_size: i64,
    spawn_chance: f32,
    min_radius: f32,
    max_radius: f32,
    salt: u64,
}

impl OreDefinition {
    fn scan_margin(self) -> i64 {
        self.max_radius.ceil() as i64
    }
}

pub(super) fn generate_ores_for_chunk(
    chunk: &mut Chunk,
    position: ChunkPos,
    seed: u64,
    sampler: BiomeSampler,
) {
    let bounds = ChunkWorldBounds::new(position);
    let generator = OreGenerator::new(seed);

    for kind in OreKind::ALL {
        let definition = kind.definition();
        let margin = definition.scan_margin();
        let min_cell_x = (bounds.min_x - margin).div_euclid(definition.cell_size);
        let max_cell_x = (bounds.max_x + margin).div_euclid(definition.cell_size);
        let min_cell_y = (bounds.min_y - margin).div_euclid(definition.cell_size);
        let max_cell_y = (bounds.max_y + margin).div_euclid(definition.cell_size);
        let min_cell_z = (bounds.min_z - margin).div_euclid(definition.cell_size);
        let max_cell_z = (bounds.max_z + margin).div_euclid(definition.cell_size);

        for cell_y in min_cell_y..=max_cell_y {
            for cell_z in min_cell_z..=max_cell_z {
                for cell_x in min_cell_x..=max_cell_x {
                    let Some(vein) = generator.vein_at(kind, cell_x, cell_y, cell_z) else {
                        continue;
                    };
                    if vein.intersects(bounds) {
                        vein.paint(chunk, bounds, sampler);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct OreGenerator {
    seed: u64,
}

impl OreGenerator {
    const fn new(seed: u64) -> Self {
        Self { seed }
    }

    fn vein_at(self, kind: OreKind, cell_x: i64, cell_y: i64, cell_z: i64) -> Option<OreVein> {
        let definition = kind.definition();
        let roll_hash = hash_3d(self.seed ^ definition.salt, cell_x, cell_y, cell_z);
        if unit_f32(roll_hash) >= definition.spawn_chance {
            return None;
        }

        let center_x = cell_x * definition.cell_size
            + cell_offset(
                hash_3d(
                    self.seed ^ definition.salt.rotate_left(7),
                    cell_x,
                    cell_y,
                    cell_z,
                ),
                definition.cell_size,
            );
        let center_y = cell_y * definition.cell_size
            + cell_offset(
                hash_3d(
                    self.seed ^ definition.salt.rotate_left(19),
                    cell_x,
                    cell_y,
                    cell_z,
                ),
                definition.cell_size,
            );
        let center_z = cell_z * definition.cell_size
            + cell_offset(
                hash_3d(
                    self.seed ^ definition.salt.rotate_left(37),
                    cell_x,
                    cell_y,
                    cell_z,
                ),
                definition.cell_size,
            );
        if !(i64::from(definition.min_y)..=i64::from(definition.max_y)).contains(&center_y) {
            return None;
        }

        let radius_range = definition.max_radius - definition.min_radius;
        let radius_x = definition.min_radius
            + unit_f32(hash_3d(
                self.seed ^ definition.salt.rotate_left(11),
                cell_x,
                cell_y,
                cell_z,
            )) * radius_range;
        let radius_y = definition.min_radius
            + unit_f32(hash_3d(
                self.seed ^ definition.salt.rotate_left(29),
                cell_x,
                cell_y,
                cell_z,
            )) * radius_range
                * 0.75;
        let radius_z = definition.min_radius
            + unit_f32(hash_3d(
                self.seed ^ definition.salt.rotate_left(43),
                cell_x,
                cell_y,
                cell_z,
            )) * radius_range;

        Some(OreVein {
            kind,
            center_x,
            center_y,
            center_z,
            radius_x,
            radius_y,
            radius_z,
            shape_seed: self.seed ^ definition.salt.rotate_left(53),
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct OreVein {
    kind: OreKind,
    center_x: i64,
    center_y: i64,
    center_z: i64,
    radius_x: f32,
    radius_y: f32,
    radius_z: f32,
    shape_seed: u64,
}

impl OreVein {
    fn extent_x(self) -> i64 {
        self.radius_x.ceil() as i64
    }

    fn extent_y(self) -> i64 {
        self.radius_y.ceil() as i64
    }

    fn extent_z(self) -> i64 {
        self.radius_z.ceil() as i64
    }

    fn intersects(self, bounds: ChunkWorldBounds) -> bool {
        self.center_x + self.extent_x() >= bounds.min_x
            && self.center_x - self.extent_x() <= bounds.max_x
            && self.center_y + self.extent_y() >= bounds.min_y
            && self.center_y - self.extent_y() <= bounds.max_y
            && self.center_z + self.extent_z() >= bounds.min_z
            && self.center_z - self.extent_z() <= bounds.max_z
    }

    fn contains(self, world_x: i64, world_y: i64, world_z: i64) -> bool {
        let definition = self.kind.definition();
        if !(i64::from(definition.min_y)..=i64::from(definition.max_y)).contains(&world_y) {
            return false;
        }

        let delta_x = (world_x - self.center_x) as f32 / self.radius_x;
        let delta_y = (world_y - self.center_y) as f32 / self.radius_y;
        let delta_z = (world_z - self.center_z) as f32 / self.radius_z;
        let distance_squared = delta_x * delta_x + delta_y * delta_y + delta_z * delta_z;
        let irregularity = unit_f32(hash_3d(self.shape_seed, world_x, world_y, world_z));
        distance_squared <= 0.82 + irregularity * 0.28
    }

    fn paint(self, chunk: &mut Chunk, bounds: ChunkWorldBounds, sampler: BiomeSampler) {
        let min_x = (self.center_x - self.extent_x()).max(bounds.min_x);
        let max_x = (self.center_x + self.extent_x()).min(bounds.max_x);
        let min_y = (self.center_y - self.extent_y()).max(bounds.min_y);
        let max_y = (self.center_y + self.extent_y()).min(bounds.max_y);
        let min_z = (self.center_z - self.extent_z()).max(bounds.min_z);
        let max_z = (self.center_z + self.extent_z()).min(bounds.max_z);
        let ore = self.kind.definition().block;

        for world_y in min_y..=max_y {
            for world_z in min_z..=max_z {
                for world_x in min_x..=max_x {
                    if !self.contains(world_x, world_y, world_z) {
                        continue;
                    }
                    let local = bounds
                        .local_position(world_x, world_y, world_z)
                        .expect("clipped ore voxel must be inside the target chunk");
                    let terrain_height = sampler.sample(world_x, world_z).terrain_height;
                    if chunk.get_local(local) == BlockId::STONE
                        && world_y <= i64::from(terrain_height - ORE_SURFACE_CLEARANCE)
                    {
                        chunk.set_local(local, ore);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ChunkWorldBounds {
    min_x: i64,
    max_x: i64,
    min_y: i64,
    max_y: i64,
    min_z: i64,
    max_z: i64,
}

impl ChunkWorldBounds {
    fn new(position: ChunkPos) -> Self {
        let min_x = i64::from(position.x) * CHUNK_WIDTH as i64;
        let min_y = i64::from(position.y) * CHUNK_HEIGHT as i64;
        let min_z = i64::from(position.z) * CHUNK_DEPTH as i64;
        Self {
            min_x,
            max_x: min_x + CHUNK_WIDTH as i64 - 1,
            min_y,
            max_y: min_y + CHUNK_HEIGHT as i64 - 1,
            min_z,
            max_z: min_z + CHUNK_DEPTH as i64 - 1,
        }
    }

    fn local_position(self, world_x: i64, world_y: i64, world_z: i64) -> Option<LocalBlockPos> {
        LocalBlockPos::checked(
            usize::try_from(world_x - self.min_x).ok()?,
            usize::try_from(world_y - self.min_y).ok()?,
            usize::try_from(world_z - self.min_z).ok()?,
        )
    }
}

fn cell_offset(hash: u64, cell_size: i64) -> i64 {
    (hash % cell_size as u64) as i64
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn generated_stone_chunk(position: ChunkPos, seed: u64) -> Chunk {
        let mut chunk = Chunk::new(BlockId::STONE);
        generate_ores_for_chunk(&mut chunk, position, seed, BiomeSampler::new(seed));
        chunk
    }

    fn find_crossing_coal_vein(seed: u64) -> (OreVein, i64) {
        let generator = OreGenerator::new(seed);
        for cell_y in -6_i64..=-2 {
            for cell_z in -64_i64..=64 {
                for cell_x in -64_i64..=64 {
                    let Some(vein) = generator.vein_at(OreKind::Coal, cell_x, cell_y, cell_z)
                    else {
                        continue;
                    };
                    let chunk_x = vein.center_x.div_euclid(CHUNK_WIDTH as i64);
                    let boundaries = [
                        chunk_x * CHUNK_WIDTH as i64,
                        (chunk_x + 1) * CHUNK_WIDTH as i64,
                    ];
                    for boundary_x in boundaries {
                        if vein.contains(boundary_x - 1, vein.center_y, vein.center_z)
                            && vein.contains(boundary_x, vein.center_y, vein.center_z)
                        {
                            return (vein, boundary_x);
                        }
                    }
                }
            }
        }
        panic!("expected a deterministic coal vein crossing a chunk boundary");
    }

    #[test]
    fn coal_is_more_common_and_iron_is_deeper() {
        let coal = OreKind::Coal.definition();
        let iron = OreKind::Iron.definition();

        assert!(coal.spawn_chance > iron.spawn_chance);
        assert!(coal.max_y > iron.max_y);
        assert!(coal.min_y > iron.min_y);
    }

    #[test]
    fn ore_never_replaces_air_or_dirt() {
        let seed = 42;
        for fill in [BlockId::AIR, BlockId::DIRT] {
            let mut chunk = Chunk::new(fill);
            generate_ores_for_chunk(
                &mut chunk,
                ChunkPos::new(0, -2, 0),
                seed,
                BiomeSampler::new(seed),
            );
            assert!(chunk.blocks().iter().all(|&block| block == fill));
        }
    }

    #[test]
    fn generated_ore_is_deterministic() {
        let position = ChunkPos::new(-5, -2, 9);
        let first = generated_stone_chunk(position, 987_654);
        let second = generated_stone_chunk(position, 987_654);

        assert_eq!(first.blocks(), second.blocks());
    }

    #[test]
    fn one_vein_paints_both_sides_of_a_chunk_boundary() {
        let seed = 42;
        let sampler = BiomeSampler::new(seed);
        let (vein, boundary_x) = find_crossing_coal_vein(seed);
        let y = vein.center_y.div_euclid(CHUNK_HEIGHT as i64) as i32;
        let z = vein.center_z.div_euclid(CHUNK_DEPTH as i64) as i32;
        let left_position =
            ChunkPos::new((boundary_x - 1).div_euclid(CHUNK_WIDTH as i64) as i32, y, z);
        let right_position = ChunkPos::new(boundary_x.div_euclid(CHUNK_WIDTH as i64) as i32, y, z);
        let mut left = Chunk::new(BlockId::STONE);
        let mut right = Chunk::new(BlockId::STONE);
        let left_bounds = ChunkWorldBounds::new(left_position);
        let right_bounds = ChunkWorldBounds::new(right_position);

        vein.paint(&mut left, left_bounds, sampler);
        vein.paint(&mut right, right_bounds, sampler);

        let left_local = left_bounds
            .local_position(boundary_x - 1, vein.center_y, vein.center_z)
            .unwrap();
        let right_local = right_bounds
            .local_position(boundary_x, vein.center_y, vein.center_z)
            .unwrap();
        assert_eq!(left.get_local(left_local), BlockId::COAL_ORE);
        assert_eq!(right.get_local(right_local), BlockId::COAL_ORE);
    }

    #[test]
    fn coal_is_observed_more_often_than_iron() {
        let seed = 123;
        let mut coal_count = 0;
        let mut iron_count = 0;

        for y in -4..=0 {
            for z in -4..=4 {
                for x in -4..=4 {
                    let chunk = generated_stone_chunk(ChunkPos::new(x, y, z), seed);
                    for &block in chunk.blocks() {
                        coal_count += (block == BlockId::COAL_ORE) as usize;
                        iron_count += (block == BlockId::IRON_ORE) as usize;
                    }
                }
            }
        }

        assert!(coal_count > iron_count);
        assert!(iron_count > 0);
    }

    #[test]
    fn iron_does_not_generate_above_its_depth_limit() {
        let chunk = generated_stone_chunk(ChunkPos::new(0, 1, 0), 42);

        assert!(!chunk.blocks().contains(&BlockId::IRON_ORE));
    }

    #[test]
    fn adjacent_chunks_match_in_any_generation_order() {
        let seed = 55;
        let positions = [ChunkPos::new(-1, -2, 0), ChunkPos::new(0, -2, 0)];
        let forward = positions
            .iter()
            .map(|&position| {
                (
                    position,
                    generated_stone_chunk(position, seed).blocks().to_vec(),
                )
            })
            .collect::<HashMap<_, _>>();
        let reverse = positions
            .iter()
            .rev()
            .map(|&position| {
                (
                    position,
                    generated_stone_chunk(position, seed).blocks().to_vec(),
                )
            })
            .collect::<HashMap<_, _>>();

        assert_eq!(forward, reverse);
    }
}
