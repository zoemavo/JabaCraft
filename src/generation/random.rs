//! Stateless coordinate hashing for order-independent procedural features.

pub(super) fn hash_2d(seed: u64, x: i64, z: i64) -> u64 {
    avalanche(
        seed ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F),
    )
}

pub(super) fn hash_3d(seed: u64, x: i64, y: i64, z: i64) -> u64 {
    avalanche(
        seed ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (y as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F),
    )
}

pub(super) fn unit_f32(hash: u64) -> f32 {
    ((hash >> 40) as f32) / ((1_u32 << 24) - 1) as f32
}

fn avalanche(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_hashing_is_repeatable_and_axis_sensitive() {
        let original = hash_3d(42, -7, 11, 23);

        assert_eq!(original, hash_3d(42, -7, 11, 23));
        assert_ne!(original, hash_3d(42, -6, 11, 23));
        assert_ne!(original, hash_3d(42, -7, 12, 23));
        assert_ne!(original, hash_3d(42, -7, 11, 24));
        assert_ne!(original, hash_3d(43, -7, 11, 23));
    }

    #[test]
    fn unit_conversion_stays_normalized() {
        for hash in [0, 1, u64::MAX / 2, u64::MAX] {
            assert!((0.0..=1.0).contains(&unit_f32(hash)));
        }
    }
}
