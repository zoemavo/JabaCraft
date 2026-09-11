//! Order-independent world features that can span several chunks.

use crate::{
    block::BlockId,
    chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk},
    coordinates::{ChunkPos, LocalBlockPos},
};

use super::{
    Biome, BiomeSampler,
    random::{hash_2d, unit_f32},
};

const TREE_MIN_HEIGHT: i32 = 4;
const TREE_MAX_HEIGHT: i32 = 6;
const TREE_CANOPY_RADIUS: i64 = 2;
const TREE_SALT: u64 = 0xA7D3_5C91_EB40_268F;

pub(super) const MAX_TREE_BLOCKS_ABOVE_SURFACE: i32 = TREE_MAX_HEIGHT + 1;

/// Generates every deterministic feature that intersects `position`.
///
/// Candidate origins are sampled in world space beyond the chunk boundary by
/// the maximum feature radius. Consequently, a neighboring chunk reconstructs
/// exactly the same feature regardless of which chunk was loaded first.
pub(super) fn generate_features_for_chunk(
    chunk: &mut Chunk,
    position: ChunkPos,
    seed: u64,
    sampler: BiomeSampler,
) {
    let bounds = ChunkWorldBounds::new(position);
    let generator = FeatureGenerator::new(seed, sampler);

    for root_z in bounds.min_z - TREE_CANOPY_RADIUS..=bounds.max_z + TREE_CANOPY_RADIUS {
        for root_x in bounds.min_x - TREE_CANOPY_RADIUS..=bounds.max_x + TREE_CANOPY_RADIUS {
            let Some(tree) = generator.tree_at(root_x, root_z) else {
                continue;
            };
            if tree.intersects(bounds) {
                tree.paint(chunk, bounds);
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FeatureGenerator {
    seed: u64,
    sampler: BiomeSampler,
}

impl FeatureGenerator {
    const fn new(seed: u64, sampler: BiomeSampler) -> Self {
        Self { seed, sampler }
    }

    fn tree_at(self, root_x: i64, root_z: i64) -> Option<TreeFeature> {
        let sample = self.sampler.sample(root_x, root_z);
        if sample.terrain_height < super::terrain::SEA_LEVEL
            || !matches!(sample.biome, Biome::Forest | Biome::Plains)
            || sample.biome.definition().surface_block != BlockId::GRASS
            || !passes_density_roll(self.seed, root_x, root_z, sample.tree_density)
            || !self.has_suitable_ground(root_x, root_z, sample.terrain_height)
        {
            return None;
        }

        Some(TreeFeature {
            root_x,
            surface_y: sample.terrain_height,
            root_z,
            trunk_height: tree_height(self.seed, root_x, root_z),
        })
    }

    fn has_suitable_ground(self, root_x: i64, root_z: i64, surface_y: i32) -> bool {
        for offset_z in -TREE_CANOPY_RADIUS..=TREE_CANOPY_RADIUS {
            for offset_x in -TREE_CANOPY_RADIUS..=TREE_CANOPY_RADIUS {
                let neighbor_height = self
                    .sampler
                    .sample(root_x + offset_x, root_z + offset_z)
                    .terrain_height;

                // The inner 3x3 area supports the trunk. The wider check keeps
                // the lower canopy out of a neighboring cliff face.
                if (offset_x.abs() <= 1
                    && offset_z.abs() <= 1
                    && (neighbor_height - surface_y).abs() > 1)
                    || neighbor_height > surface_y + 1
                {
                    return false;
                }
            }
        }
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TreeFeature {
    root_x: i64,
    surface_y: i32,
    root_z: i64,
    trunk_height: i32,
}

impl TreeFeature {
    fn trunk_top(self) -> i64 {
        i64::from(self.surface_y + self.trunk_height)
    }

    fn min_y(self) -> i64 {
        i64::from(self.surface_y + 1)
    }

    fn max_y(self) -> i64 {
        self.trunk_top() + 1
    }

    fn intersects(self, bounds: ChunkWorldBounds) -> bool {
        self.root_x + TREE_CANOPY_RADIUS >= bounds.min_x
            && self.root_x - TREE_CANOPY_RADIUS <= bounds.max_x
            && self.root_z + TREE_CANOPY_RADIUS >= bounds.min_z
            && self.root_z - TREE_CANOPY_RADIUS <= bounds.max_z
            && self.max_y() >= bounds.min_y
            && self.min_y() <= bounds.max_y
    }

    fn block_at(self, world_x: i64, world_y: i64, world_z: i64) -> Option<BlockId> {
        if world_x == self.root_x
            && world_z == self.root_z
            && (self.min_y()..=self.trunk_top()).contains(&world_y)
        {
            return Some(BlockId::WOOD);
        }

        let offset_x = world_x - self.root_x;
        let offset_y = world_y - self.trunk_top();
        let offset_z = world_z - self.root_z;
        let inside_canopy = (-TREE_CANOPY_RADIUS..=TREE_CANOPY_RADIUS).contains(&offset_x)
            && (-2..=1).contains(&offset_y)
            && (-TREE_CANOPY_RADIUS..=TREE_CANOPY_RADIUS).contains(&offset_z)
            && offset_x.abs() + offset_z.abs() + offset_y.abs() / 2 <= 3;
        inside_canopy.then_some(BlockId::LEAVES)
    }

    fn paint(self, chunk: &mut Chunk, bounds: ChunkWorldBounds) {
        let min_x = (self.root_x - TREE_CANOPY_RADIUS).max(bounds.min_x);
        let max_x = (self.root_x + TREE_CANOPY_RADIUS).min(bounds.max_x);
        let min_y = self.min_y().max(bounds.min_y);
        let max_y = self.max_y().min(bounds.max_y);
        let min_z = (self.root_z - TREE_CANOPY_RADIUS).max(bounds.min_z);
        let max_z = (self.root_z + TREE_CANOPY_RADIUS).min(bounds.max_z);

        for world_y in min_y..=max_y {
            for world_z in min_z..=max_z {
                for world_x in min_x..=max_x {
                    let Some(block) = self.block_at(world_x, world_y, world_z) else {
                        continue;
                    };
                    let local = bounds
                        .local_position(world_x, world_y, world_z)
                        .expect("clipped feature voxel must be inside the target chunk");
                    let current = chunk.get_local(local);
                    let can_replace = match block {
                        BlockId::LEAVES => matches!(current, BlockId::AIR | BlockId::LEAVES),
                        BlockId::WOOD => {
                            matches!(current, BlockId::AIR | BlockId::LEAVES | BlockId::WOOD)
                        }
                        _ => false,
                    };
                    if can_replace {
                        chunk.set_local(local, block);
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
        let local_x = world_x - self.min_x;
        let local_y = world_y - self.min_y;
        let local_z = world_z - self.min_z;
        LocalBlockPos::checked(
            usize::try_from(local_x).ok()?,
            usize::try_from(local_y).ok()?,
            usize::try_from(local_z).ok()?,
        )
    }
}

fn passes_density_roll(seed: u64, x: i64, z: i64, density: f32) -> bool {
    density > 0.0 && unit_f32(hash_2d(seed ^ TREE_SALT, x, z)) < density
}

fn tree_height(seed: u64, x: i64, z: i64) -> i32 {
    let height_range = (TREE_MAX_HEIGHT - TREE_MIN_HEIGHT + 1) as u64;
    TREE_MIN_HEIGHT + (hash_2d(seed ^ TREE_SALT.rotate_left(17), x, z) % height_range) as i32
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::generation::{CaveSettings, terrain::generate_terrain_chunk};

    fn generated_chunk(position: ChunkPos, seed: u64) -> Chunk {
        generate_terrain_chunk(position, seed, CaveSettings::default())
    }

    fn find_border_tree(seed: u64) -> TreeFeature {
        let generator = FeatureGenerator::new(seed, BiomeSampler::new(seed));
        for chunk_x in -64_i64..=64 {
            let root_x = chunk_x * CHUNK_WIDTH as i64 + CHUNK_WIDTH as i64 - 1;
            for root_z in -2_048_i64..=2_048 {
                if let Some(tree) = generator.tree_at(root_x, root_z)
                    && generator.tree_at(root_x + 1, root_z).is_none()
                {
                    return tree;
                }
            }
        }
        panic!("expected a deterministic tree on a searched chunk edge");
    }

    #[test]
    fn trees_only_originate_in_forest_or_plains() {
        let seed = 7;
        let sampler = BiomeSampler::new(seed);
        let generator = FeatureGenerator::new(seed, sampler);
        let mut checked_desert = false;
        let mut checked_rocky = false;

        for z in (-4_096_i64..=4_096).step_by(17) {
            for x in (-4_096_i64..=4_096).step_by(17) {
                match sampler.sample(x, z).biome {
                    Biome::Desert => {
                        checked_desert = true;
                        assert!(generator.tree_at(x, z).is_none());
                    }
                    Biome::Rocky => {
                        checked_rocky = true;
                        assert!(generator.tree_at(x, z).is_none());
                    }
                    Biome::Plains | Biome::Forest => {}
                }
                if checked_desert && checked_rocky {
                    return;
                }
            }
        }
        panic!("test search must encounter Desert and Rocky");
    }

    #[test]
    fn both_forest_and_plains_produce_deterministic_trees() {
        let seed = 42;
        let sampler = BiomeSampler::new(seed);
        let generator = FeatureGenerator::new(seed, sampler);
        let mut found_forest = false;
        let mut found_plains = false;

        for z in -2_048_i64..=2_048 {
            for x in (-2_048_i64..=2_048).step_by(3) {
                if generator.tree_at(x, z).is_some() {
                    match sampler.sample(x, z).biome {
                        Biome::Forest => found_forest = true,
                        Biome::Plains => found_plains = true,
                        Biome::Desert | Biome::Rocky => {
                            panic!("tree originated in a forbidden biome")
                        }
                    }
                }
                if found_forest && found_plains {
                    return;
                }
            }
        }
        panic!("search must find deterministic trees in Forest and Plains");
    }

    #[test]
    fn accepted_tree_has_supported_grass_ground_and_clear_cliff_margin() {
        let seed = 42;
        let sampler = BiomeSampler::new(seed);
        let tree = find_border_tree(seed);
        let root = sampler.sample(tree.root_x, tree.root_z);

        assert!(matches!(root.biome, Biome::Forest | Biome::Plains));
        assert_eq!(root.biome.definition().surface_block, BlockId::GRASS);
        assert_eq!(tree.surface_y, root.terrain_height);
        assert!(FeatureGenerator::new(seed, sampler).has_suitable_ground(
            tree.root_x,
            tree.root_z,
            tree.surface_y
        ));
        assert_eq!(
            tree.block_at(tree.root_x, i64::from(tree.surface_y), tree.root_z),
            None
        );
        assert_eq!(
            tree.block_at(tree.root_x, i64::from(tree.surface_y + 1), tree.root_z),
            Some(BlockId::WOOD)
        );
    }

    #[test]
    fn canopy_crosses_chunk_edge_without_overwriting_terrain() {
        let seed = 42;
        let tree = find_border_tree(seed);
        let neighbor_x = tree.root_x + 1;
        let canopy_y = tree.trunk_top();
        let position = ChunkPos::new(
            neighbor_x.div_euclid(CHUNK_WIDTH as i64) as i32,
            canopy_y.div_euclid(CHUNK_HEIGHT as i64) as i32,
            tree.root_z.div_euclid(CHUNK_DEPTH as i64) as i32,
        );
        let chunk = generated_chunk(position, seed);
        let bounds = ChunkWorldBounds::new(position);
        let canopy_local = bounds
            .local_position(neighbor_x, canopy_y, tree.root_z)
            .unwrap();

        assert_eq!(chunk.get_local(canopy_local), BlockId::LEAVES);

        let root_position = ChunkPos::new(
            tree.root_x.div_euclid(CHUNK_WIDTH as i64) as i32,
            tree.surface_y.div_euclid(CHUNK_HEIGHT as i32),
            tree.root_z.div_euclid(CHUNK_DEPTH as i64) as i32,
        );
        let root_chunk = generated_chunk(root_position, seed);
        let root_local = ChunkWorldBounds::new(root_position)
            .local_position(tree.root_x, i64::from(tree.surface_y), tree.root_z)
            .unwrap();
        assert_eq!(root_chunk.get_local(root_local), BlockId::GRASS);
    }

    #[test]
    fn neighboring_chunks_are_identical_in_any_generation_order() {
        let seed = 123;
        let tree = find_border_tree(seed);
        let root_chunk_x = tree.root_x.div_euclid(CHUNK_WIDTH as i64) as i32;
        let chunk_z = tree.root_z.div_euclid(CHUNK_DEPTH as i64) as i32;
        let min_chunk_y = tree.min_y().div_euclid(CHUNK_HEIGHT as i64) as i32;
        let max_chunk_y = tree.max_y().div_euclid(CHUNK_HEIGHT as i64) as i32;
        let positions = (min_chunk_y..=max_chunk_y)
            .flat_map(|y| {
                [
                    ChunkPos::new(root_chunk_x, y, chunk_z),
                    ChunkPos::new(root_chunk_x + 1, y, chunk_z),
                ]
            })
            .collect::<Vec<_>>();

        let forward = positions
            .iter()
            .map(|&position| (position, generated_chunk(position, seed).blocks().to_vec()))
            .collect::<HashMap<_, _>>();
        let reverse = positions
            .iter()
            .rev()
            .map(|&position| (position, generated_chunk(position, seed).blocks().to_vec()))
            .collect::<HashMap<_, _>>();

        assert_eq!(forward, reverse);
    }
}
