use std::collections::VecDeque;

use crate::{
    block::{BlockFace, BlockRegistry},
    chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, ChunkStorage},
    coordinates::{ChunkPos, WorldBlockPos},
    generation::terrain_height_at,
};

pub(super) const MAX_LIGHT_LEVEL: u8 = 15;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct LightLevels {
    pub sky: u8,
    pub block: u8,
}

pub(super) struct ChunkLightMap {
    bounds: LightBounds,
    sky: Box<[u8]>,
    block: Box<[u8]>,
    transparent: Box<[u8]>,
}

impl ChunkLightMap {
    pub(super) fn build(
        target: ChunkPos,
        storage: &ChunkStorage,
        registry: &BlockRegistry,
        terrain_seed: u64,
    ) -> Self {
        let bounds = LightBounds::around_chunk(target);
        let mut sky = vec![0; bounds.volume()];
        let mut block = vec![0; bounds.volume()];
        let mut transparent = vec![0; bounds.volume()];
        let mut ceilings = vec![0; bounds.width * bounds.depth];
        let mut block_queue = VecDeque::new();

        // Predict unloaded terrain once per column. The previous implementation
        // recalculated procedural noise during every flood-fill neighbor visit.
        for local_z in 0..bounds.depth {
            for local_x in 0..bounds.width {
                let world_x = bounds.min.x + local_x as i32;
                let world_z = bounds.min.z + local_z as i32;
                let ceiling =
                    terrain_height_at(terrain_seed, i64::from(world_x), i64::from(world_z));
                ceilings[bounds.column_index(local_x, local_z)] = ceiling;

                for local_y in 0..bounds.height {
                    let world_y = bounds.min.y + local_y as i32;
                    let index = bounds.local_index(local_x, local_y, local_z);
                    transparent[index] = u8::from(world_y > ceiling);
                }
            }
        }

        // Overlay loaded voxel data chunk-by-chunk, avoiding a hash-table lookup
        // for every single light cell in the dense volume.
        let (min_chunk, _) = bounds.min.split();
        let (max_chunk, _) = bounds.max().split();
        for chunk_y in min_chunk.y..=max_chunk.y {
            for chunk_z in min_chunk.z..=max_chunk.z {
                for chunk_x in min_chunk.x..=max_chunk.x {
                    let chunk_position = ChunkPos::new(chunk_x, chunk_y, chunk_z);
                    let Some(chunk) = storage.get_chunk(chunk_position) else {
                        continue;
                    };
                    let chunk_origin = WorldBlockPos::new(
                        chunk_x * CHUNK_WIDTH as i32,
                        chunk_y * CHUNK_HEIGHT as i32,
                        chunk_z * CHUNK_DEPTH as i32,
                    );
                    let x_start = (bounds.min.x - chunk_origin.x).max(0) as usize;
                    let y_start = (bounds.min.y - chunk_origin.y).max(0) as usize;
                    let z_start = (bounds.min.z - chunk_origin.z).max(0) as usize;
                    let x_end =
                        (bounds.max().x - chunk_origin.x).min(CHUNK_WIDTH as i32 - 1) as usize;
                    let y_end =
                        (bounds.max().y - chunk_origin.y).min(CHUNK_HEIGHT as i32 - 1) as usize;
                    let z_end =
                        (bounds.max().z - chunk_origin.z).min(CHUNK_DEPTH as i32 - 1) as usize;

                    for y in y_start..=y_end {
                        for z in z_start..=z_end {
                            for x in x_start..=x_end {
                                let local = crate::coordinates::LocalBlockPos::new(x, y, z)
                                    .expect("overlap stays inside chunk bounds");
                                let voxel = chunk.get_local(local);
                                let world = chunk_position.world_block(local);
                                let index = bounds
                                    .index(world)
                                    .expect("overlapping chunk cell stays inside light bounds");
                                let is_transparent = registry.is_transparent(voxel);
                                transparent[index] = u8::from(is_transparent);

                                if !is_transparent {
                                    let local_x = (world.x - bounds.min.x) as usize;
                                    let local_z = (world.z - bounds.min.z) as usize;
                                    let column = bounds.column_index(local_x, local_z);
                                    ceilings[column] = ceilings[column].max(world.y);
                                }

                                let emission = registry.emitted_light(voxel).min(MAX_LIGHT_LEVEL);
                                if emission > block[index] {
                                    block[index] = emission;
                                    block_queue.push_back(index);
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut sky_queue = VecDeque::new();
        for local_z in 0..bounds.depth {
            for local_x in 0..bounds.width {
                let ceiling = ceilings[bounds.column_index(local_x, local_z)];
                let first_sky_y = (ceiling + 1).max(bounds.min.y);
                for world_y in first_sky_y..=bounds.max().y {
                    let local_y = (world_y - bounds.min.y) as usize;
                    let index = bounds.local_index(local_x, local_y, local_z);
                    if transparent[index] != 0 {
                        sky[index] = MAX_LIGHT_LEVEL;
                        sky_queue.push_back(index);
                    }
                }
            }
        }

        propagate_channel(&mut sky, &transparent, bounds, sky_queue);
        propagate_channel(&mut block, &transparent, bounds, block_queue);

        Self {
            bounds,
            sky: sky.into_boxed_slice(),
            block: block.into_boxed_slice(),
            transparent: transparent.into_boxed_slice(),
        }
    }

    pub(super) fn sample(&self, position: WorldBlockPos) -> LightLevels {
        self.bounds
            .index(position)
            .map_or_else(LightLevels::default, |index| LightLevels {
                sky: self.sky[index],
                block: self.block[index],
            })
    }

    fn is_opaque(&self, position: WorldBlockPos) -> bool {
        self.bounds
            .index(position)
            .is_some_and(|index| self.transparent[index] == 0)
    }
}

pub(super) fn vertex_light_color(
    block: WorldBlockPos,
    face: BlockFace,
    vertex: [f32; 3],
    lights: &ChunkLightMap,
) -> [f32; 4] {
    let normal = face_normal(face);
    let outside = offset(block, normal);
    let [side_a, side_b] = vertex_side_offsets(normal, vertex);
    let corner = add_offset(side_a, side_b);
    let samples = [
        outside,
        offset(outside, side_a),
        offset(outside, side_b),
        offset(outside, corner),
    ];

    let mut rgb = [0.0; 3];
    for sample in samples {
        let sample_rgb = light_rgb(lights.sample(sample));
        for channel in 0..3 {
            rgb[channel] += sample_rgb[channel] * 0.25;
        }
    }

    let side_a_blocked = lights.is_opaque(offset(outside, side_a));
    let side_b_blocked = lights.is_opaque(offset(outside, side_b));
    let corner_blocked = lights.is_opaque(offset(outside, corner));
    let occlusion = if side_a_blocked && side_b_blocked {
        3
    } else {
        u8::from(side_a_blocked) + u8::from(side_b_blocked) + u8::from(corner_blocked)
    };
    let ao = [1.0, 0.86, 0.72, 0.58][usize::from(occlusion)];
    let shade = face_shade(face);

    for channel in &mut rgb {
        *channel = (*channel * ao * shade).clamp(0.0, 1.0);
    }
    [rgb[0], rgb[1], rgb[2], 1.0]
}

fn propagate_channel(
    levels: &mut [u8],
    transparent: &[u8],
    bounds: LightBounds,
    mut queue: VecDeque<usize>,
) {
    while let Some(index) = queue.pop_front() {
        let level = levels[index];
        if level <= 1 {
            continue;
        }
        let next_level = level - 1;
        let local_x = index % bounds.width;
        let yz = index / bounds.width;
        let local_z = yz % bounds.depth;
        let local_y = yz / bounds.depth;

        if local_x > 0 {
            spread_to(index - 1, next_level, levels, transparent, &mut queue);
        }
        if local_x + 1 < bounds.width {
            spread_to(index + 1, next_level, levels, transparent, &mut queue);
        }
        if local_z > 0 {
            spread_to(
                index - bounds.width,
                next_level,
                levels,
                transparent,
                &mut queue,
            );
        }
        if local_z + 1 < bounds.depth {
            spread_to(
                index + bounds.width,
                next_level,
                levels,
                transparent,
                &mut queue,
            );
        }
        let layer_stride = bounds.width * bounds.depth;
        if local_y > 0 {
            spread_to(
                index - layer_stride,
                next_level,
                levels,
                transparent,
                &mut queue,
            );
        }
        if local_y + 1 < bounds.height {
            spread_to(
                index + layer_stride,
                next_level,
                levels,
                transparent,
                &mut queue,
            );
        }
    }
}

fn spread_to(
    index: usize,
    level: u8,
    levels: &mut [u8],
    transparent: &[u8],
    queue: &mut VecDeque<usize>,
) {
    if transparent[index] != 0 && levels[index] < level {
        levels[index] = level;
        queue.push_back(index);
    }
}

fn light_rgb(levels: LightLevels) -> [f32; 3] {
    let sky = light_curve(levels.sky);
    let block = light_curve(levels.block);
    [
        (0.035 + sky * 0.965 + block).min(1.0),
        (0.040 + sky * 0.940 + block * 0.72).min(1.0),
        (0.055 + sky * 0.945 + block * 0.38).min(1.0),
    ]
}

fn light_curve(level: u8) -> f32 {
    const CURVE: [f32; 16] = [
        0.0,
        0.025_839_07,
        0.065_866_91,
        0.113_865_06,
        0.167_902_74,
        0.226_927_07,
        0.290_255_84,
        0.357_403_78,
        0.428_004_44,
        0.501_769_4,
        0.578_464_6,
        0.657_895_5,
        0.739_897_4,
        0.824_328_54,
        0.911_065_6,
        1.0,
    ];
    CURVE[usize::from(level.min(MAX_LIGHT_LEVEL))]
}

fn face_normal(face: BlockFace) -> [i32; 3] {
    match face {
        BlockFace::Top => [0, 1, 0],
        BlockFace::Bottom => [0, -1, 0],
        BlockFace::North => [0, 0, -1],
        BlockFace::South => [0, 0, 1],
        BlockFace::East => [1, 0, 0],
        BlockFace::West => [-1, 0, 0],
    }
}

fn face_shade(face: BlockFace) -> f32 {
    match face {
        BlockFace::Top => 1.0,
        BlockFace::Bottom => 0.50,
        BlockFace::North | BlockFace::South => 0.80,
        BlockFace::East | BlockFace::West => 0.65,
    }
}

fn vertex_side_offsets(normal: [i32; 3], vertex: [f32; 3]) -> [[i32; 3]; 2] {
    let mut sides = [[0; 3]; 2];
    let mut side_index = 0;
    for axis in 0..3 {
        if normal[axis] != 0 {
            continue;
        }
        sides[side_index][axis] = if vertex[axis] < 0.5 { -1 } else { 1 };
        side_index += 1;
    }
    sides
}

fn offset(position: WorldBlockPos, delta: [i32; 3]) -> WorldBlockPos {
    WorldBlockPos::new(
        position.x + delta[0],
        position.y + delta[1],
        position.z + delta[2],
    )
}

fn add_offset(left: [i32; 3], right: [i32; 3]) -> [i32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

#[derive(Clone, Copy)]
struct LightBounds {
    min: WorldBlockPos,
    width: usize,
    height: usize,
    depth: usize,
}

impl LightBounds {
    fn around_chunk(chunk: ChunkPos) -> Self {
        let radius = i32::from(MAX_LIGHT_LEVEL);
        Self {
            min: WorldBlockPos::new(
                chunk.x * CHUNK_WIDTH as i32 - radius,
                chunk.y * CHUNK_HEIGHT as i32 - radius,
                chunk.z * CHUNK_DEPTH as i32 - radius,
            ),
            width: CHUNK_WIDTH + usize::from(MAX_LIGHT_LEVEL) * 2,
            height: CHUNK_HEIGHT + usize::from(MAX_LIGHT_LEVEL) * 2,
            depth: CHUNK_DEPTH + usize::from(MAX_LIGHT_LEVEL) * 2,
        }
    }

    #[cfg(test)]
    fn from_min_max(min: WorldBlockPos, max: WorldBlockPos) -> Self {
        Self {
            min,
            width: (max.x - min.x + 1) as usize,
            height: (max.y - min.y + 1) as usize,
            depth: (max.z - min.z + 1) as usize,
        }
    }

    fn max(self) -> WorldBlockPos {
        WorldBlockPos::new(
            self.min.x + self.width as i32 - 1,
            self.min.y + self.height as i32 - 1,
            self.min.z + self.depth as i32 - 1,
        )
    }

    fn volume(self) -> usize {
        self.width * self.height * self.depth
    }

    fn column_index(self, local_x: usize, local_z: usize) -> usize {
        local_z * self.width + local_x
    }

    fn local_index(self, local_x: usize, local_y: usize, local_z: usize) -> usize {
        (local_y * self.depth + local_z) * self.width + local_x
    }

    fn index(self, position: WorldBlockPos) -> Option<usize> {
        let local_x = position.x - self.min.x;
        let local_y = position.y - self.min.y;
        let local_z = position.z - self.min.z;
        if local_x < 0
            || local_y < 0
            || local_z < 0
            || local_x >= self.width as i32
            || local_y >= self.height as i32
            || local_z >= self.depth as i32
        {
            return None;
        }

        Some(self.local_index(local_x as usize, local_y as usize, local_z as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_loses_one_level_per_block() {
        let bounds = LightBounds::from_min_max(
            WorldBlockPos::new(-20, -20, -20),
            WorldBlockPos::new(20, 20, 20),
        );
        let transparent = vec![1; bounds.volume()];
        let mut levels = vec![0; bounds.volume()];
        let origin = bounds.index(WorldBlockPos::new(0, 0, 0)).unwrap();
        levels[origin] = MAX_LIGHT_LEVEL;

        propagate_channel(&mut levels, &transparent, bounds, VecDeque::from([origin]));

        assert_eq!(
            levels[bounds.index(WorldBlockPos::new(1, 0, 0)).unwrap()],
            14
        );
        assert_eq!(
            levels[bounds.index(WorldBlockPos::new(14, 0, 0)).unwrap()],
            1
        );
        assert_eq!(
            levels[bounds.index(WorldBlockPos::new(15, 0, 0)).unwrap()],
            0
        );
    }

    #[test]
    fn opaque_cells_stop_propagation() {
        let bounds =
            LightBounds::from_min_max(WorldBlockPos::new(0, 0, 0), WorldBlockPos::new(4, 0, 0));
        let wall = WorldBlockPos::new(2, 0, 0);
        let source = bounds.index(WorldBlockPos::new(0, 0, 0)).unwrap();
        let wall_index = bounds.index(wall).unwrap();
        let mut transparent = vec![1; bounds.volume()];
        transparent[wall_index] = 0;
        let mut levels = vec![0; bounds.volume()];
        levels[source] = 5;

        propagate_channel(&mut levels, &transparent, bounds, VecDeque::from([source]));

        assert_eq!(
            levels[bounds.index(WorldBlockPos::new(1, 0, 0)).unwrap()],
            4
        );
        assert_eq!(levels[wall_index], 0);
        assert_eq!(
            levels[bounds.index(WorldBlockPos::new(3, 0, 0)).unwrap()],
            0
        );
    }
}
