//! Small deterministic coherent-noise maps used by procedural generation.

#[derive(Clone, Copy, Debug)]
pub(super) struct NoiseMap {
    seed: u64,
    frequency: f64,
    octaves: u8,
    persistence: f64,
    lacunarity: f64,
}

impl NoiseMap {
    pub(super) const fn new(seed: u64, frequency: f64, octaves: u8) -> Self {
        Self {
            seed,
            frequency,
            octaves,
            persistence: 0.5,
            lacunarity: 2.0,
        }
    }

    /// Samples normalized fractal value noise in approximately `-1.0..=1.0`.
    pub(super) fn sample(self, world_x: i64, world_z: i64) -> f32 {
        let mut frequency = self.frequency;
        let mut amplitude = 1.0;
        let mut value = 0.0;
        let mut amplitude_sum = 0.0;

        for octave in 0..self.octaves {
            value += value_noise(
                self.seed.wrapping_add(u64::from(octave)),
                world_x as f64 * frequency,
                world_z as f64 * frequency,
            ) * amplitude;
            amplitude_sum += amplitude;
            amplitude *= self.persistence;
            frequency *= self.lacunarity;
        }

        if amplitude_sum == 0.0 {
            0.0
        } else {
            (value / amplitude_sum).clamp(-1.0, 1.0) as f32
        }
    }

    /// Samples normalized coherent noise using all three world axes.
    pub(super) fn sample_3d(self, world_x: i64, world_y: i64, world_z: i64) -> f32 {
        let mut frequency = self.frequency;
        let mut amplitude = 1.0;
        let mut value = 0.0;
        let mut amplitude_sum = 0.0;

        for octave in 0..self.octaves {
            value += value_noise_3d(
                self.seed.wrapping_add(u64::from(octave)),
                world_x as f64 * frequency,
                world_y as f64 * frequency,
                world_z as f64 * frequency,
            ) * amplitude;
            amplitude_sum += amplitude;
            amplitude *= self.persistence;
            frequency *= self.lacunarity;
        }

        if amplitude_sum == 0.0 {
            0.0
        } else {
            (value / amplitude_sum).clamp(-1.0, 1.0) as f32
        }
    }
}

fn value_noise(seed: u64, x: f64, z: f64) -> f64 {
    let x0 = x.floor() as i64;
    let z0 = z.floor() as i64;
    let tx = smoothstep(x - x0 as f64);
    let tz = smoothstep(z - z0 as f64);

    let bottom = lerp(
        lattice_value(seed, x0, z0),
        lattice_value(seed, x0 + 1, z0),
        tx,
    );
    let top = lerp(
        lattice_value(seed, x0, z0 + 1),
        lattice_value(seed, x0 + 1, z0 + 1),
        tx,
    );
    lerp(bottom, top, tz)
}

fn value_noise_3d(seed: u64, x: f64, y: f64, z: f64) -> f64 {
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let z0 = z.floor() as i64;
    let tx = smoothstep(x - x0 as f64);
    let ty = smoothstep(y - y0 as f64);
    let tz = smoothstep(z - z0 as f64);

    let lower_z0 = lerp(
        lattice_value_3d(seed, x0, y0, z0),
        lattice_value_3d(seed, x0 + 1, y0, z0),
        tx,
    );
    let lower_z1 = lerp(
        lattice_value_3d(seed, x0, y0, z0 + 1),
        lattice_value_3d(seed, x0 + 1, y0, z0 + 1),
        tx,
    );
    let upper_z0 = lerp(
        lattice_value_3d(seed, x0, y0 + 1, z0),
        lattice_value_3d(seed, x0 + 1, y0 + 1, z0),
        tx,
    );
    let upper_z1 = lerp(
        lattice_value_3d(seed, x0, y0 + 1, z0 + 1),
        lattice_value_3d(seed, x0 + 1, y0 + 1, z0 + 1),
        tx,
    );
    let lower = lerp(lower_z0, lower_z1, tz);
    let upper = lerp(upper_z0, upper_z1, tz);
    lerp(lower, upper, ty)
}

fn lattice_value(seed: u64, x: i64, z: i64) -> f64 {
    let mixed = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    let random = splitmix64(mixed);
    let unit = (random >> 11) as f64 / ((1_u64 << 53) - 1) as f64;
    unit * 2.0 - 1.0
}

fn lattice_value_3d(seed: u64, x: i64, y: i64, z: i64) -> f64 {
    let mixed = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93)
        ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    let random = splitmix64(mixed);
    let unit = (random >> 11) as f64 / ((1_u64 << 53) - 1) as f64;
    unit * 2.0 - 1.0
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

fn lerp(start: f64, end: f64, amount: f64) -> f64 {
    start + (end - start) * amount
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_deterministic_and_normalized() {
        let map = NoiseMap::new(42, 1.0 / 128.0, 4);

        for (x, z) in [(-10_000, 50), (-1, -1), (0, 0), (321, -789)] {
            let first = map.sample(x, z);
            assert_eq!(first, map.sample(x, z));
            assert!((-1.0..=1.0).contains(&first));
        }
    }

    #[test]
    fn independent_maps_do_not_share_the_same_samples() {
        let temperature = NoiseMap::new(1, 1.0 / 320.0, 3);
        let humidity = NoiseMap::new(2, 1.0 / 320.0, 3);

        assert!((temperature.sample(123, -456) - humidity.sample(123, -456)).abs() > f32::EPSILON);
    }

    #[test]
    fn interpolation_is_continuous_across_lattice_boundaries() {
        let map = NoiseMap::new(99, 1.0 / 16.0, 1);
        let before = map.sample(15, 37);
        let boundary = map.sample(16, 37);
        let after = map.sample(17, 37);

        assert!((before - boundary).abs() < 0.1);
        assert!((boundary - after).abs() < 0.1);
    }

    #[test]
    fn three_dimensional_noise_uses_every_axis_deterministically() {
        let map = NoiseMap::new(123, 1.0 / 32.0, 3);
        let original = map.sample_3d(17, -29, 43);

        assert_eq!(original, map.sample_3d(17, -29, 43));
        assert_ne!(original, map.sample_3d(18, -29, 43));
        assert_ne!(original, map.sample_3d(17, -28, 43));
        assert_ne!(original, map.sample_3d(17, -29, 44));
        assert!((-1.0..=1.0).contains(&original));
    }
}
