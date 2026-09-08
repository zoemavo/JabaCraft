use bevy::prelude::*;

use crate::item::ItemId;

const ICON_SIZE: f32 = 32.0;
const ATLAS_COLUMNS: u32 = 4;

#[derive(Resource)]
pub(super) struct ItemIconAssets {
    image: Handle<Image>,
}

pub(super) fn load_item_icons(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(ItemIconAssets {
        image: asset_server.load("textures/faithful_32x_items.png"),
    });
}

pub(super) fn set_item_icon(image: &mut ImageNode, icons: &ItemIconAssets, item: Option<ItemId>) {
    image.image = icons.image.clone();
    let Some(item) = item else {
        image.color = Color::NONE;
        image.rect = None;
        return;
    };

    let index = u32::from(item.as_u16());
    let column = index % ATLAS_COLUMNS;
    let row = index / ATLAS_COLUMNS;
    let min = Vec2::new(column as f32 * ICON_SIZE, row as f32 * ICON_SIZE);

    image.color = Color::WHITE;
    image.rect = Some(Rect::from_corners(min, min + Vec2::splat(ICON_SIZE)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_item_maps_inside_the_icon_atlas() {
        for item in ItemId::ALL {
            let index = u32::from(item.as_u16());
            assert!(index < 12);
            assert!(index % ATLAS_COLUMNS < ATLAS_COLUMNS);
            assert!(index / ATLAS_COLUMNS < 3);
        }
    }
}
