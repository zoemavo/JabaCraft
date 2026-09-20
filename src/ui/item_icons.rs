use bevy::{asset::AssetLoadFailedEvent, prelude::*};

use crate::item::{BUILTIN_ITEM_COUNT, ItemId, ItemRegistry, ItemStack};

const PLACEHOLDER_PATH: &str = "textures/items/placeholder.png";
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

#[derive(Clone, Copy, Debug, PartialEq)]
struct ItemIconDefinition {
    path: &'static str,
    tint: [f32; 4],
}

const ITEM_ICONS: [ItemIconDefinition; BUILTIN_ITEM_COUNT] = [
    icon("textures/items/grass_block.png"),
    icon("textures/items/dirt_block.png"),
    icon("textures/items/stone_block.png"),
    icon("textures/items/sand_block.png"),
    icon("textures/items/oak_log.png"),
    tinted("textures/items/oak_leaves.png", [0.34, 0.64, 0.22, 1.0]),
    tinted("textures/items/water.png", [0.22, 0.46, 0.88, 0.82]),
    icon("textures/items/coal_ore.png"),
    icon("textures/items/iron_ore.png"),
    icon("textures/items/stick.png"),
    icon("textures/items/apple.png"),
    icon("textures/items/oak_planks.png"),
    icon("textures/items/stone_pickaxe.png"),
    icon("textures/items/wooden_pickaxe.png"),
    icon("textures/items/iron_pickaxe.png"),
    icon("textures/items/wooden_axe.png"),
    icon("textures/items/stone_axe.png"),
    icon("textures/items/iron_axe.png"),
    icon("textures/items/wooden_shovel.png"),
    icon("textures/items/stone_shovel.png"),
    icon("textures/items/iron_shovel.png"),
    icon("textures/items/wooden_sword.png"),
    icon("textures/items/stone_sword.png"),
    icon("textures/items/iron_sword.png"),
];

const fn icon(path: &'static str) -> ItemIconDefinition {
    tinted(path, WHITE)
}

const fn tinted(path: &'static str, tint: [f32; 4]) -> ItemIconDefinition {
    ItemIconDefinition { path, tint }
}

fn definition(item: ItemId) -> ItemIconDefinition {
    ITEM_ICONS[item.as_u16() as usize]
}

#[derive(Resource)]
pub(super) struct ItemIconAssets {
    icons: [Handle<Image>; BUILTIN_ITEM_COUNT],
    placeholder: Handle<Image>,
    missing: [bool; BUILTIN_ITEM_COUNT],
}

impl ItemIconAssets {
    fn image_and_tint(&self, item: ItemId) -> (&Handle<Image>, [f32; 4]) {
        let index = item.as_u16() as usize;
        let tint = if self.missing[index] {
            WHITE
        } else {
            definition(item).tint
        };
        (&self.icons[index], tint)
    }
}

pub(super) fn load_item_icons(mut commands: Commands, asset_server: Res<AssetServer>) {
    let icons = ItemId::ALL.map(|item| asset_server.load(definition(item).path));
    commands.insert_resource(ItemIconAssets {
        icons,
        placeholder: asset_server.load(PLACEHOLDER_PATH),
        missing: [false; BUILTIN_ITEM_COUNT],
    });
}

pub(super) fn log_missing_item_icons(
    mut failures: MessageReader<AssetLoadFailedEvent<Image>>,
    assets: Option<ResMut<ItemIconAssets>>,
    registry: Res<ItemRegistry>,
) {
    let Some(mut assets) = assets else { return };
    for failure in failures.read() {
        if failure.id == assets.placeholder.id() {
            error!(
                "Faithful item placeholder is missing at {}: {}",
                PLACEHOLDER_PATH, failure.error
            );
            continue;
        }

        for item in ItemId::ALL {
            let index = item.as_u16() as usize;
            if assets.icons[index].id() != failure.id || assets.missing[index] {
                continue;
            }
            error!(
                "Missing Faithful icon for {} at {}: {}. Using {}",
                registry.definition(item).name,
                definition(item).path,
                failure.error,
                PLACEHOLDER_PATH
            );
            assets.icons[index] = assets.placeholder.clone();
            assets.missing[index] = true;
        }
    }
}

pub(super) fn set_item_icon(image: &mut ImageNode, icons: &ItemIconAssets, item: Option<ItemId>) {
    let Some(item) = item else {
        image.image = Handle::default();
        image.color = Color::NONE;
        image.rect = None;
        return;
    };

    let (handle, [r, g, b, a]) = icons.image_and_tint(item);
    image.image = handle.clone();
    image.color = Color::srgba(r, g, b, a);
    image.rect = None;
}

pub(super) fn durability_fraction(
    stack: Option<ItemStack>,
    registry: &ItemRegistry,
) -> Option<f32> {
    let stack = stack?;
    let tool = registry.tool(stack.item())?;
    Some(stack.durability_fraction(tool.durability).clamp(0.0, 1.0))
}

pub(super) fn durability_color(fraction: f32) -> Color {
    let fraction = fraction.clamp(0.0, 1.0);
    Color::srgb(1.0 - fraction, fraction, 0.0)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_builtin_item_has_a_png_lookup() {
        let paths = ItemId::ALL
            .map(|item| definition(item).path)
            .into_iter()
            .collect::<HashSet<_>>();

        assert_eq!(paths.len(), BUILTIN_ITEM_COUNT);
        assert!(paths.into_iter().all(|path| path.ends_with(".png")));
    }

    #[test]
    fn different_tools_use_different_faithful_files() {
        assert_ne!(
            definition(ItemId::WOODEN_PICKAXE).path,
            definition(ItemId::STONE_PICKAXE).path
        );
        assert_ne!(
            definition(ItemId::STONE_PICKAXE).path,
            definition(ItemId::IRON_PICKAXE).path
        );
        assert_ne!(
            definition(ItemId::IRON_AXE).path,
            definition(ItemId::IRON_SWORD).path
        );
    }

    #[test]
    fn durability_lookup_only_returns_a_bar_for_tools() {
        let registry = ItemRegistry::default();
        let tool = registry.create_stack(ItemId::STONE_PICKAXE, 1).unwrap();
        let block = registry.create_stack(ItemId::STONE_BLOCK, 1).unwrap();

        assert_eq!(durability_fraction(Some(tool), &registry), Some(1.0));
        assert_eq!(durability_fraction(Some(block), &registry), None);
        assert_eq!(durability_fraction(None, &registry), None);
    }
}
