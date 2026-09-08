//! Data-driven shapeless crafting recipes and atomic inventory transactions.

use bevy::prelude::*;

use crate::{
    inventory::PlayerInventory,
    item::{ItemId, ItemRegistry},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecipeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ingredient {
    pub item: ItemId,
    pub count: u32,
}

impl Ingredient {
    pub const fn new(item: ItemId, count: u32) -> Self {
        Self { item, count }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recipe {
    pub name: &'static str,
    pub inputs: &'static [Ingredient],
    pub outputs: &'static [Ingredient],
}

const WOOD_TO_PLANKS_INPUTS: &[Ingredient] = &[Ingredient::new(ItemId::WOOD_BLOCK, 1)];
const WOOD_TO_PLANKS_OUTPUTS: &[Ingredient] = &[Ingredient::new(ItemId::OAK_PLANKS, 4)];
const PLANKS_TO_STICKS_INPUTS: &[Ingredient] = &[Ingredient::new(ItemId::OAK_PLANKS, 2)];
const PLANKS_TO_STICKS_OUTPUTS: &[Ingredient] = &[Ingredient::new(ItemId::STICK, 4)];
const STONE_PICKAXE_INPUTS: &[Ingredient] = &[
    Ingredient::new(ItemId::STONE_BLOCK, 3),
    Ingredient::new(ItemId::STICK, 2),
];
const STONE_PICKAXE_OUTPUTS: &[Ingredient] = &[Ingredient::new(ItemId::STONE_PICKAXE, 1)];

#[derive(Debug, Resource)]
pub struct RecipeRegistry {
    recipes: Vec<Recipe>,
}

impl RecipeRegistry {
    pub fn recipes(&self) -> &[Recipe] {
        &self.recipes
    }

    pub fn get(&self, id: RecipeId) -> Option<&Recipe> {
        self.recipes.get(id.0)
    }
}

impl Default for RecipeRegistry {
    fn default() -> Self {
        Self {
            recipes: vec![
                Recipe {
                    name: "1 Oak Log -> 4 Planks",
                    inputs: WOOD_TO_PLANKS_INPUTS,
                    outputs: WOOD_TO_PLANKS_OUTPUTS,
                },
                Recipe {
                    name: "2 Planks -> 4 Sticks",
                    inputs: PLANKS_TO_STICKS_INPUTS,
                    outputs: PLANKS_TO_STICKS_OUTPUTS,
                },
                Recipe {
                    name: "3 Stone + 2 Sticks -> Pickaxe",
                    inputs: STONE_PICKAXE_INPUTS,
                    outputs: STONE_PICKAXE_OUTPUTS,
                },
            ],
        }
    }
}

pub fn can_craft(inventory: &PlayerInventory, recipe: &Recipe) -> bool {
    recipe.inputs.iter().all(|input| {
        let total_required = recipe
            .inputs
            .iter()
            .filter(|candidate| candidate.item == input.item)
            .map(|candidate| candidate.count)
            .sum();
        inventory.item_count(input.item) >= total_required
    })
}

/// Performs the recipe on a clone and commits only when every input can be
/// consumed and every output fits. Failure therefore cannot lose or duplicate items.
pub fn craft(inventory: &mut PlayerInventory, recipe: &Recipe, items: &ItemRegistry) -> bool {
    if !can_craft(inventory, recipe) {
        return false;
    }

    let mut transaction = inventory.clone();
    for input in recipe.inputs {
        if transaction.remove_item(input.item, input.count) != input.count {
            return false;
        }
    }
    for output in recipe.outputs {
        if transaction.add_item(output.item, output.count, items) != 0 {
            return false;
        }
    }

    *inventory = transaction;
    true
}

pub struct CraftingPlugin;

impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RecipeRegistry>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(index: usize) -> Recipe {
        RecipeRegistry::default().recipes[index]
    }

    #[test]
    fn wood_recipe_consumes_one_log_and_produces_four_planks() {
        let items = ItemRegistry::default();
        let mut inventory = PlayerInventory::default();
        inventory.add_item(ItemId::WOOD_BLOCK, 2, &items);

        assert!(craft(&mut inventory, &recipe(0), &items));
        assert_eq!(inventory.item_count(ItemId::WOOD_BLOCK), 1);
        assert_eq!(inventory.item_count(ItemId::OAK_PLANKS), 4);
    }

    #[test]
    fn pickaxe_recipe_consumes_every_required_resource_exactly() {
        let items = ItemRegistry::default();
        let mut inventory = PlayerInventory::default();
        inventory.add_item(ItemId::STONE_BLOCK, 4, &items);
        inventory.add_item(ItemId::STICK, 3, &items);

        assert!(craft(&mut inventory, &recipe(2), &items));
        assert_eq!(inventory.item_count(ItemId::STONE_BLOCK), 1);
        assert_eq!(inventory.item_count(ItemId::STICK), 1);
        assert_eq!(inventory.item_count(ItemId::STONE_PICKAXE), 1);
    }

    #[test]
    fn planks_recipe_consumes_two_planks_and_produces_four_sticks() {
        let items = ItemRegistry::default();
        let mut inventory = PlayerInventory::default();
        inventory.add_item(ItemId::OAK_PLANKS, 3, &items);

        assert!(craft(&mut inventory, &recipe(1), &items));
        assert_eq!(inventory.item_count(ItemId::OAK_PLANKS), 1);
        assert_eq!(inventory.item_count(ItemId::STICK), 4);
    }

    #[test]
    fn missing_inputs_leave_inventory_unchanged() {
        let items = ItemRegistry::default();
        let mut inventory = PlayerInventory::default();
        inventory.add_item(ItemId::STONE_BLOCK, 2, &items);
        inventory.add_item(ItemId::STICK, 2, &items);
        let before = inventory.clone();

        assert!(!craft(&mut inventory, &recipe(2), &items));
        assert_eq!(inventory.slots(), before.slots());
    }

    #[test]
    fn outputs_that_do_not_fit_roll_back_consumed_inputs() {
        let items = ItemRegistry::default();
        let mut inventory = PlayerInventory::default();
        inventory.add_item(ItemId::WOOD_BLOCK, 64, &items);
        inventory.add_item(ItemId::DIRT_BLOCK, 64 * 35, &items);
        let before = inventory.clone();

        assert!(!craft(&mut inventory, &recipe(0), &items));
        assert_eq!(inventory.slots(), before.slots());
    }

    #[test]
    fn repeated_input_entries_are_counted_together() {
        const DUPLICATE_INPUTS: &[Ingredient] = &[
            Ingredient::new(ItemId::WOOD_BLOCK, 1),
            Ingredient::new(ItemId::WOOD_BLOCK, 1),
        ];
        const OUTPUTS: &[Ingredient] = &[Ingredient::new(ItemId::OAK_PLANKS, 1)];
        let recipe = Recipe {
            name: "Duplicate input test",
            inputs: DUPLICATE_INPUTS,
            outputs: OUTPUTS,
        };
        let items = ItemRegistry::default();
        let mut inventory = PlayerInventory::default();
        inventory.add_item(ItemId::WOOD_BLOCK, 1, &items);

        assert!(!can_craft(&inventory, &recipe));
        assert!(!craft(&mut inventory, &recipe, &items));
        assert_eq!(inventory.item_count(ItemId::WOOD_BLOCK), 1);
    }
}
