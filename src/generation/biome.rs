//! Biome classification and terrain parameters sampled in world space.

use crate::block::BlockId;

use super::noise::NoiseMap;

const TEMPERATURE_SALT: u64 = 0x8A5C_61E7_D4B3_209F;
const HUMIDITY_SALT: u64 = 0xD19B_04A2_76CE_5381;
const TERRAIN_SALT: u64 = 0x4F23_B8D1_C965_7A0E;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Biome {
    Plains,
    Forest,
    Desert,
    Rocky,
}

impl Biome {
    pub const ALL: [Self; 4] = [Self::Plains, Self::Forest, Self::Desert, Self::Rocky];

    pub const fn definition(self) -> BiomeDefinition {
        match self {
            Self::Plains => BiomeDefinition {
                surface_block: BlockId::GRASS,
                subsurface_block: BlockId::DIRT,
                base_height: 4.0,
                height_variation: 3.0,
                tree_density: 0.006,
            },
            Self::Forest => BiomeDefinition {
                surface_block: BlockId::GRASS,
                subsurface_block: BlockId::DIRT,
                base_height: 7.0,
                height_variation: 5.0,
                tree_density: 0.035,
            },
            Self::Desert => BiomeDefinition {
                surface_block: BlockId::SAND,
                subsurface_block: BlockId::SAND,
                base_height: 3.0,
                height_variation: 4.0,
                tree_density: 0.0,
            },
            Self::Rocky => BiomeDefinition {
                surface_block: BlockId::STONE,
                subsurface_block: BlockId::STONE,
                base_height: 12.0,
                height_variation: 14.0,
                tree_density: 0.0,
            },
        }
    }

    const fn climate_center(self) -> (f32, f32) {
        match self {
            Self::Plains => (0.0, 0.0),
            Self::Forest => (-0.05, 0.65),
            Self::Desert => (0.65, -0.55),
            Self::Rocky => (-0.65, -0.35),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiomeDefinition {
    pub surface_block: BlockId,
    pub subsurface_block: BlockId,
    pub base_height: f32,
    pub height_variation: f32,
    /// Expected fraction of world columns containing a tree origin.
    pub tree_density: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiomeSample {
    pub biome: Biome,
    pub temperature: f32,
    pub humidity: f32,
    pub terrain_height: i32,
    pub tree_density: f32,
}

/// Owns independent climate maps and derives a biome per world-space column.
#[derive(Clone, Copy, Debug)]
pub struct BiomeSampler {
    temperature: NoiseMap,
    humidity: NoiseMap,
    terrain: NoiseMap,
}

impl BiomeSampler {
    pub fn new(seed: u64) -> Self {
        Self {
            temperature: NoiseMap::new(seed ^ TEMPERATURE_SALT, 1.0 / 384.0, 3),
            humidity: NoiseMap::new(seed ^ HUMIDITY_SALT, 1.0 / 320.0, 3),
            terrain: NoiseMap::new(seed ^ TERRAIN_SALT, 1.0 / 96.0, 4),
        }
    }

    pub fn sample(self, world_x: i64, world_z: i64) -> BiomeSample {
        let temperature = self.temperature.sample(world_x, world_z);
        let humidity = self.humidity.sample(world_x, world_z);
        let (biome, weights) = classify_and_weight(temperature, humidity);

        let (base_height, height_variation) = Biome::ALL.into_iter().zip(weights).fold(
            (0.0, 0.0),
            |accumulator, (candidate, weight)| {
                let definition = candidate.definition();
                (
                    accumulator.0 + definition.base_height * weight,
                    accumulator.1 + definition.height_variation * weight,
                )
            },
        );
        let terrain_height =
            (base_height + self.terrain.sample(world_x, world_z) * height_variation).round() as i32;

        BiomeSample {
            biome,
            temperature,
            humidity,
            terrain_height,
            tree_density: biome.definition().tree_density,
        }
    }
}

fn classify_and_weight(temperature: f32, humidity: f32) -> (Biome, [f32; 4]) {
    let mut nearest = Biome::Plains;
    let mut nearest_distance = f32::MAX;
    let mut raw_weights = [0.0; 4];
    let mut weight_sum = 0.0;

    for (index, biome) in Biome::ALL.into_iter().enumerate() {
        let (center_temperature, center_humidity) = biome.climate_center();
        let temperature_delta = temperature - center_temperature;
        let humidity_delta = humidity - center_humidity;
        let distance_squared =
            temperature_delta * temperature_delta + humidity_delta * humidity_delta;

        if distance_squared < nearest_distance {
            nearest = biome;
            nearest_distance = distance_squared;
        }

        // Inverse-distance blending prevents cliffs in height parameters at a
        // discrete surface-biome boundary.
        let weight = 1.0 / (0.08 + distance_squared).powi(2);
        raw_weights[index] = weight;
        weight_sum += weight;
    }

    for weight in &mut raw_weights {
        *weight /= weight_sum;
    }

    (nearest, raw_weights)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn climate_centers_select_all_required_biomes() {
        for expected in Biome::ALL {
            let (temperature, humidity) = expected.climate_center();
            assert_eq!(classify_and_weight(temperature, humidity).0, expected);
        }
    }

    #[test]
    fn biome_parameters_are_visibly_distinct() {
        assert_eq!(Biome::Plains.definition().surface_block, BlockId::GRASS);
        assert_eq!(Biome::Forest.definition().surface_block, BlockId::GRASS);
        assert_eq!(Biome::Desert.definition().surface_block, BlockId::SAND);
        assert_eq!(Biome::Rocky.definition().surface_block, BlockId::STONE);
        assert!(Biome::Forest.definition().tree_density > Biome::Plains.definition().tree_density);
        assert!(Biome::Plains.definition().tree_density > 0.0);
        assert_eq!(Biome::Desert.definition().tree_density, 0.0);
        assert_eq!(Biome::Rocky.definition().tree_density, 0.0);
        assert!(
            Biome::Rocky.definition().height_variation
                > Biome::Desert.definition().height_variation
        );
    }

    #[test]
    fn samples_are_deterministic_for_seed_and_world_coordinates() {
        let sampler = BiomeSampler::new(0xCAFE_BABE);
        let first = sampler.sample(-1_234, 5_678);

        assert_eq!(first, sampler.sample(-1_234, 5_678));
        assert_ne!(first, BiomeSampler::new(0xCAFE_BABF).sample(-1_234, 5_678));
    }

    #[test]
    fn biome_boundaries_are_not_locked_to_chunk_edges() {
        let sampler = BiomeSampler::new(42);
        let mut found_non_chunk_boundary = false;
        let mut observed = HashSet::new();

        for z in (-2_048..=2_048).step_by(31) {
            let mut previous = sampler.sample(-2_048, z).biome;
            observed.insert(previous);
            for x in -2_047..=2_048 {
                let current = sampler.sample(x, z).biome;
                observed.insert(current);
                if current != previous && x.rem_euclid(16) != 0 {
                    found_non_chunk_boundary = true;
                    break;
                }
                previous = current;
            }
            if found_non_chunk_boundary && observed.len() == Biome::ALL.len() {
                break;
            }
        }

        assert!(found_non_chunk_boundary);
        assert_eq!(observed.len(), Biome::ALL.len());
    }
}
