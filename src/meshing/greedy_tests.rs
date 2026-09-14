use super::*;
use crate::chunk::Chunk;
use std::collections::BTreeMap;

fn fixture(kind: &str) -> (ChunkPos, ChunkStorage) {
    let position = ChunkPos::new(-2, 4, -3);
    let mut chunk = Chunk::default();
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let block = match kind {
                    "solid" => BlockId::STONE,
                    "slab" if y == 0 => BlockId::STONE,
                    "checkerboard" if (x + y + z) % 2 == 0 => BlockId::STONE,
                    "mixed" if y < 3 => {
                        if x < 8 {
                            BlockId::DIRT
                        } else {
                            BlockId::STONE
                        }
                    }
                    "mixed" if y == 3 && x < 5 => BlockId::WATER,
                    "mixed" if y == 4 && x == 5 => BlockId::LEAVES,
                    "terrain" if y <= ((x * 3 + z * 5) % 11) => {
                        if y < 3 {
                            BlockId::STONE
                        } else {
                            BlockId::GRASS
                        }
                    }
                    _ => BlockId::AIR,
                };
                chunk.set_block(x, y, z, block).unwrap();
            }
        }
    }
    let mut storage = ChunkStorage::default();
    storage.insert_chunk(position, chunk);
    (position, storage)
}

// Expand large quads into unit faces, retaining the exact face-corner light/AO.
// This checks coverage, direction, tile assignment and shading independently of
// vertex/index ordering. Every unit face must occur exactly once.
fn coverage(mesh: &ChunkMeshBuffers) -> BTreeMap<[i32; 6], (u16, [[u32; 4]; 4])> {
    let mut result = BTreeMap::new();
    for first in (0..mesh.positions.len()).step_by(4) {
        let points = &mesh.positions[first..first + 4];
        let normal = mesh.normals[first];
        let axis = normal.iter().position(|n| *n != 0.0).unwrap();
        let a = if axis == 0 { 1 } else { 0 };
        let b = if axis == 2 { 1 } else { 2 };
        let min: [i32; 3] =
            std::array::from_fn(|i| points.iter().map(|p| p[i] as i32).min().unwrap());
        let max: [i32; 3] =
            std::array::from_fn(|i| points.iter().map(|p| p[i] as i32).max().unwrap());
        let texture = (mesh.uvs[first][0] * 4.0).floor() as u16
            + 4 * (mesh.uvs[first][1] * 4.0).floor() as u16;
        let colors = std::array::from_fn(|v| mesh.colors[first + v].map(f32::to_bits));
        for u in min[a]..max[a] {
            for v in min[b]..max[b] {
                let mut p = min;
                p[a] = u;
                p[b] = v;
                assert!(
                    result
                        .insert(
                            [
                                p[0],
                                p[1],
                                p[2],
                                normal[0] as i32,
                                normal[1] as i32,
                                normal[2] as i32
                            ],
                            (texture, colors)
                        )
                        .is_none()
                );
            }
        }
        for triangle in [[0, 1, 2], [0, 2, 3]] {
            let p0 = bevy::prelude::Vec3::from_array(points[triangle[0]]);
            let p1 = bevy::prelude::Vec3::from_array(points[triangle[1]]);
            let p2 = bevy::prelude::Vec3::from_array(points[triangle[2]]);
            assert!(
                (p1 - p0)
                    .cross(p2 - p0)
                    .dot(bevy::prelude::Vec3::from_array(normal))
                    > 0.0
            );
        }
        assert!(mesh.normals[first..first + 4].iter().all(|n| *n == normal));
    }
    assert_eq!(mesh.indices.len(), mesh.positions.len() / 4 * 6);
    assert!(
        mesh.indices
            .iter()
            .all(|i| (*i as usize) < mesh.positions.len())
    );
    result
}

#[test]
fn greedy_preserves_coverage_normals_winding_materials_and_vertex_lighting() {
    let registry = BlockRegistry::default();
    for name in ["solid", "slab", "checkerboard", "mixed", "terrain"] {
        let (position, storage) = fixture(name);
        let before = build_chunk_mesh_buffers_naive(position, &storage, &registry, 7);
        let after = build_chunk_mesh_buffers_with_seed(position, &storage, &registry, 7);
        assert_eq!(coverage(&before.opaque), coverage(&after.opaque), "{name}");
        assert_eq!(
            coverage(&before.water),
            coverage(&after.water),
            "{name} water"
        );
        assert_eq!(before.water.positions.len(), after.water.positions.len());
        assert!(after.opaque.positions.len() <= before.opaque.positions.len());
        assert!(after.opaque.indices.len() <= before.opaque.indices.len());
    }
}

#[test]
fn slabs_and_solid_chunks_reduce_geometry_and_chunk_seams_stay_culled() {
    let registry = BlockRegistry::default();
    for name in ["solid", "slab"] {
        let (position, mut storage) = fixture(name);
        let before = build_chunk_mesh_buffers_naive(position, &storage, &registry, 0);
        let after = build_chunk_mesh_buffers_with_seed(position, &storage, &registry, 0);
        assert!(
            after.opaque.positions.len() < before.opaque.positions.len() / 2,
            "{name}"
        );
        storage.insert_chunk(
            ChunkPos::new(position.x - 1, position.y, position.z),
            Chunk::new(BlockId::STONE),
        );
        let before = build_chunk_mesh_buffers_naive(position, &storage, &registry, 0);
        let after = build_chunk_mesh_buffers_with_seed(position, &storage, &registry, 0);
        assert_eq!(coverage(&before.opaque), coverage(&after.opaque));
        assert!(!after.opaque.normals.contains(&WEST.normal));
    }
}

#[test]
fn repeating_atlas_coordinates_match_old_unit_faces_on_every_axis() {
    for face in FACES {
        for texture in 1..16 {
            let mut mesh = ChunkMeshBuffers::default();
            let mut extent = [3.0, 5.0, 7.0];
            let normal_axis = face.normal.iter().position(|n| *n != 0.0).unwrap();
            extent[normal_axis] = 1.0;
            mesh.push_quad([0.0; 3], face, texture, [[1.0; 4]; 4], extent);
            let length = |to: usize| {
                (0..3)
                    .map(|i| (face.vertices[to][i] - face.vertices[0][i]).abs() * extent[i])
                    .sum::<f32>()
            };
            let uv = face_uvs(texture, face.block_face);
            for (s, t) in [(0.173, 0.387), (0.731, 0.819), (0.321, 0.219)] {
                for channel in 0..2 {
                    let interpolate = |values: &[[f32; 2]], s: f32, t: f32| {
                        values[0][channel]
                            + s * (values[1][channel] - values[0][channel])
                            + t * (values[3][channel] - values[0][channel])
                    };
                    let atlas = interpolate(&mesh.uvs, s, t);
                    let repeat = interpolate(&mesh.repeats, s, t);
                    let shader =
                        ((atlas * 4.0).floor() * 32.0 + 0.5 + repeat.fract() * 31.0) / 128.0;
                    let expected =
                        interpolate(&uv, (s * length(1)).fract(), (t * length(3)).fract());
                    assert!(
                        (shader - expected).abs() < 0.00001,
                        "{:?} tile {texture}",
                        face.block_face
                    );
                }
            }
        }
    }
}

#[test]
fn merged_opaque_tiles_are_opaque_in_the_shadow_prepass() {
    let registry = BlockRegistry::default();
    let image = super::super::atlas::create_block_texture_atlas();
    let data = image.data.as_ref().unwrap();
    for block in BlockId::ALL {
        if registry.is_transparent(block) {
            continue;
        }
        for face in FACES {
            let tile = registry.texture_for(block, face.block_face) as usize;
            for y in 0..32 {
                for x in 0..32 {
                    assert_eq!(
                        data[((tile / 4 * 32 + y) * 128 + tile % 4 * 32 + x) * 4 + 3],
                        255
                    );
                }
            }
        }
    }
}

/// Reproducible full-mesher comparison including lighting and allocation.
/// cargo test greedy_benchmark -- --ignored --nocapture
#[test]
#[ignore]
fn greedy_benchmark() {
    use std::{hint::black_box, time::Instant};
    let registry = BlockRegistry::default();
    println!("fixture | vertices old/new | indices old/new | mean ms old/new (20 runs)");
    for name in ["solid", "slab", "checkerboard", "mixed", "terrain"] {
        let (position, storage) = fixture(name);
        let old = build_chunk_mesh_buffers_naive(position, &storage, &registry, 0);
        let new = build_chunk_mesh_buffers_with_seed(position, &storage, &registry, 0);
        let start = Instant::now();
        for _ in 0..20 {
            black_box(build_chunk_mesh_buffers_naive(
                position,
                black_box(&storage),
                &registry,
                0,
            ));
        }
        let old_ms = start.elapsed().as_secs_f64() * 1000.0 / 20.0;
        let start = Instant::now();
        for _ in 0..20 {
            black_box(build_chunk_mesh_buffers_with_seed(
                position,
                black_box(&storage),
                &registry,
                0,
            ));
        }
        let new_ms = start.elapsed().as_secs_f64() * 1000.0 / 20.0;
        println!(
            "{name} | {}/{} | {}/{} | {:.3}/{:.3}",
            old.opaque.positions.len(),
            new.opaque.positions.len(),
            old.opaque.indices.len(),
            new.opaque.indices.len(),
            old_ms,
            new_ms
        );
    }
}
