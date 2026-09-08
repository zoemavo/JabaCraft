use bevy::prelude::*;

use crate::{
    block::BlockRegistry, chunk::ChunkStorage, coordinates::WorldBlockPos,
    inventory::PlayerInventory, player::Player,
};

use super::{BUILTIN_ITEM_COUNT, ItemId, ItemRegistry, ItemStack};

const ITEM_SIZE: f32 = 0.24;
const ITEM_HALF_HEIGHT: f32 = ITEM_SIZE * 0.5;
const DROP_GRAVITY: f32 = 18.0;
const TERMINAL_VELOCITY: f32 = 24.0;
const PICKUP_DELAY: f32 = 0.3;
const PICKUP_RADIUS: f32 = 2.0;

#[derive(Component, Debug)]
pub struct DroppedItem {
    stack: ItemStack,
    velocity: Vec3,
    age: f32,
}

impl DroppedItem {
    pub const fn stack(&self) -> ItemStack {
        self.stack
    }
}

#[derive(Resource)]
pub struct DroppedItemAssets {
    mesh: Handle<Mesh>,
    materials: [Handle<StandardMaterial>; BUILTIN_ITEM_COUNT],
}

pub(super) fn create_dropped_item_assets(
    mut commands: Commands,
    registry: Res<ItemRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(ITEM_SIZE, ITEM_SIZE, ITEM_SIZE * 0.35));
    let materials = ItemId::ALL.map(|item| {
        let [r, g, b, _] = registry.debug_color(item);
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.78,
            ..default()
        })
    });
    commands.insert_resource(DroppedItemAssets { mesh, materials });
}

pub fn spawn_dropped_item(
    commands: &mut Commands,
    assets: &DroppedItemAssets,
    stack: ItemStack,
    position: Vec3,
) -> Entity {
    commands
        .spawn((
            Name::new("Dropped Item"),
            DroppedItem {
                stack,
                velocity: Vec3::new(0.0, 2.4, 0.0),
                age: 0.0,
            },
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.materials[stack.item().as_index()].clone()),
            Transform::from_translation(position),
        ))
        .id()
}

pub(super) fn apply_dropped_item_physics(
    time: Res<Time>,
    storage: Res<ChunkStorage>,
    blocks: Res<BlockRegistry>,
    mut drops: Query<(&mut DroppedItem, &mut Transform)>,
) {
    let delta = time.delta_secs().max(0.0);
    for (mut drop, mut transform) in &mut drops {
        drop.age += delta;
        step_drop_motion(
            &mut transform.translation,
            &mut drop.velocity,
            delta,
            &storage,
            &blocks,
        );
        transform.rotate_y(delta * 2.2);
        transform.rotate_x(delta * 0.35);
    }
}

fn step_drop_motion(
    position: &mut Vec3,
    velocity: &mut Vec3,
    delta: f32,
    storage: &ChunkStorage,
    blocks: &BlockRegistry,
) {
    velocity.y = (velocity.y - DROP_GRAVITY * delta).max(-TERMINAL_VELOCITY);
    let mut next = *position + *velocity * delta;

    if velocity.y <= 0.0 {
        let foot = next - Vec3::Y * ITEM_HALF_HEIGHT;
        let block_position = WorldBlockPos::new(
            foot.x.floor() as i32,
            foot.y.floor() as i32,
            foot.z.floor() as i32,
        );
        if storage
            .get_block(block_position)
            .is_some_and(|block| blocks.is_solid(block))
        {
            next.y = block_position.y as f32 + 1.0 + ITEM_HALF_HEIGHT;
            velocity.y = 0.0;
            velocity.x *= 0.72;
            velocity.z *= 0.72;
        }
    }

    *position = next;
}

pub(super) fn pickup_dropped_items(
    mut commands: Commands,
    player: Single<&Transform, With<Player>>,
    registry: Res<ItemRegistry>,
    mut inventory: ResMut<PlayerInventory>,
    mut drops: Query<(Entity, &Transform, &mut DroppedItem)>,
) {
    for (entity, transform, mut drop) in &mut drops {
        if !pickup_ready(
            drop.age,
            transform.translation.distance_squared(player.translation),
        ) {
            continue;
        }

        if collect_stack(&mut drop.stack, &mut inventory, &registry) {
            commands.entity(entity).despawn();
        }
    }
}

fn pickup_ready(age: f32, distance_squared: f32) -> bool {
    age >= PICKUP_DELAY && distance_squared <= PICKUP_RADIUS * PICKUP_RADIUS
}

/// Returns true only when the complete dropped stack entered the inventory.
fn collect_stack(
    stack: &mut ItemStack,
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
) -> bool {
    let remainder = inventory.add_item(stack.item(), stack.count(), registry);
    stack.remove(stack.count() - remainder);
    stack.is_empty()
}

#[cfg(test)]
mod tests {
    use crate::{block::BlockId, chunk::Chunk, coordinates::ChunkPos};

    use super::*;

    #[test]
    fn falling_drop_stops_on_top_of_solid_voxel() {
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(ChunkPos::default(), Chunk::default());
        storage
            .set_block(WorldBlockPos::new(1, 0, 1), BlockId::STONE)
            .unwrap();
        let mut position = Vec3::new(1.5, 1.4, 1.5);
        let mut velocity = Vec3::new(0.0, -10.0, 0.0);

        step_drop_motion(
            &mut position,
            &mut velocity,
            1.0 / 32.0,
            &storage,
            &BlockRegistry::default(),
        );

        assert!((position.y - (1.0 + ITEM_HALF_HEIGHT)).abs() < 0.0001);
        assert_eq!(velocity.y, 0.0);
    }

    #[test]
    fn pickup_despawns_only_after_the_whole_stack_fits() {
        let registry = ItemRegistry::default();
        let mut inventory = PlayerInventory::default();
        let mut drop = registry.create_stack(ItemId::APPLE, 5).unwrap();

        assert!(collect_stack(&mut drop, &mut inventory, &registry));
        assert_eq!(inventory.item_count(ItemId::APPLE), 5);
        assert!(drop.is_empty());
    }

    #[test]
    fn pickup_delay_and_radius_are_enforced() {
        assert!(!pickup_ready(PICKUP_DELAY - 0.001, 0.0));
        assert!(pickup_ready(PICKUP_DELAY, PICKUP_RADIUS * PICKUP_RADIUS));
        assert!(!pickup_ready(
            PICKUP_DELAY,
            PICKUP_RADIUS * PICKUP_RADIUS + 0.001
        ));
    }

    #[test]
    fn partial_pickup_keeps_the_remainder_in_the_entity_stack() {
        let registry = ItemRegistry::default();
        let mut inventory = PlayerInventory::default();
        inventory.add_item(ItemId::APPLE, 63, &registry);
        inventory.add_item(ItemId::DIRT_BLOCK, 64 * 35, &registry);
        let mut drop = registry.create_stack(ItemId::APPLE, 5).unwrap();

        assert!(!collect_stack(&mut drop, &mut inventory, &registry));
        assert_eq!(inventory.item_count(ItemId::APPLE), 64);
        assert_eq!(drop.count(), 4);
    }
}
