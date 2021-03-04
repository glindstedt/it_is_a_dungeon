use bracket_lib::prelude::*;
use specs::prelude::*;

use crate::{
    components::{
        named, AreaOfEffect, CombatStats, Confusion, Consumable, Equipable, Equipped, GivenName,
        InBackpack, InflictsDamage, Name, Position, ProvidesHealing, SufferDamage, WantsToDropItem,
        WantsToRemoveItem, WantsToUseItem,
    },
    gamelog::GameLog,
    map::Map,
};

use super::ParticleBuilder;
pub struct ItemUseSystem {}

impl<'a> System<'a> for ItemUseSystem {
    type SystemData = (
        ReadExpect<'a, Entity>,
        WriteExpect<'a, GameLog>,
        ReadExpect<'a, Map>,
        Entities<'a>,
        WriteStorage<'a, WantsToUseItem>,
        ReadStorage<'a, Name>,
        ReadStorage<'a, GivenName>,
        WriteStorage<'a, CombatStats>,
        ReadStorage<'a, Consumable>,
        ReadStorage<'a, ProvidesHealing>,
        ReadStorage<'a, InflictsDamage>,
        WriteStorage<'a, SufferDamage>,
        ReadStorage<'a, AreaOfEffect>,
        WriteStorage<'a, Confusion>,
        ReadStorage<'a, Equipable>,
        WriteStorage<'a, Equipped>,
        WriteStorage<'a, InBackpack>,
        WriteExpect<'a, ParticleBuilder>,
        ReadStorage<'a, Position>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            player_entity,
            mut gamelog,
            map,
            entities,
            mut wants_drink,
            names,
            given_names,
            mut combat_stats,
            consumables,
            healing,
            inflict_damage,
            mut suffer_damage,
            aoe,
            mut confused,
            equipable,
            mut equipped,
            mut backpack,
            mut particle_builder,
            positions
        ) = data;

        for (entity, useitem) in (&entities, &wants_drink).join() {
            let mut targets: Vec<Entity> = Vec::new();
            match useitem.target {
                None => {
                    targets.push(*player_entity);
                }
                Some(target) => {
                    let area_effect = aoe.get(useitem.item);
                    match area_effect {
                        None => {
                            let idx = map.xy_idx(target.x, target.y);
                            for mob in map.tile_content[idx].iter() {
                                targets.push(*mob);
                            }
                        }
                        Some(area_effect) => {
                            let mut blast_tiles = field_of_view(target, area_effect.radius, &*map);
                            blast_tiles.retain(|p| {
                                p.x > 0 && p.x < map.width - 1 && p.y > 0 && p.y < map.height - 1
                            });
                            for tile_idx in blast_tiles.iter() {
                                let idx = map.xy_idx(tile_idx.x, tile_idx.y);
                                for mob in map.tile_content[idx].iter() {
                                    targets.push(*mob);
                                }
                                particle_builder.request(tile_idx.x, tile_idx.y, RGB::named(ORANGE), RGB::named(BLACK), to_cp437('░'), 200.0)
                            }
                        }
                    }
                }
            }

            let item_heals = healing.get(useitem.item);
            if let Some(healer) = item_heals {
                for target in targets.iter() {
                    let stats = combat_stats.get_mut(*target);
                    if let Some(stats) = stats {
                        stats.hp = i32::min(stats.max_hp, stats.hp + healer.heal_amount);
                        if entity == *player_entity {
                            gamelog.entries.push(format!(
                                "You drink the {}, healing {} hp.",
                                names.get(useitem.item).unwrap().name,
                                healer.heal_amount
                            ));
                        }
                        if let Some(pos) = positions.get(*target) {
                            particle_builder.request(pos.x, pos.y, RGB::named(GREEN), RGB::named(BLACK), to_cp437('♥'), 200.0)
                        }
                    }
                }
            }

            let item_damages = inflict_damage.get(useitem.item);
            if let Some(damage) = item_damages {
                for mob in targets.iter() {
                    SufferDamage::new_damage(&mut suffer_damage, *mob, damage.damage);
                    if entity == *player_entity {
                        let title = named(names.get(*mob), given_names.get(*mob));
                        let item_name = names.get(useitem.item).unwrap();
                        gamelog.entries.push(format!(
                            "You use {} on {}, inflicting {} damage.",
                            item_name.name, title, damage.damage
                        ));
                        if let Some(pos) = positions.get(*mob) {
                            particle_builder.request(pos.x, pos.y, RGB::named(RED), RGB::named(BLACK), to_cp437('‼'), 200.0)
                        }
                    }
                }
            }

            let item_equipable = equipable.get(useitem.item);
            if let Some(can_equip) = item_equipable {
                let target_slot = can_equip.slot;
                let target = targets[0];

                // Remove any items the target has in the item's slot
                let mut to_unequip: Vec<Entity> = Vec::new();
                for (item_entity, already_equipped, name) in (&entities, &equipped, &names).join() {
                    if already_equipped.owner == target && already_equipped.slot == target_slot {
                        to_unequip.push(item_entity);
                        if target == *player_entity {
                            gamelog.entries.push(format!("You unequip {}.", name.name));
                        }
                    }
                }
                for item in to_unequip.iter() {
                    equipped.remove(*item);
                    backpack
                        .insert(*item, InBackpack { owner: target })
                        .expect("Unable to insert backpack entry");
                }

                // Wield the item
                equipped
                    .insert(
                        useitem.item,
                        Equipped {
                            owner: target,
                            slot: target_slot,
                        },
                    )
                    .expect("Unable to insert equipped component");
                backpack.remove(useitem.item);
                if target == *player_entity {
                    gamelog.entries.push(format!(
                        "You equip {}.",
                        names.get(useitem.item).unwrap().name
                    ));
                }
            }

            let mut add_confusion = Vec::new();
            {
                let causes_confusion = confused.get(useitem.item);
                if let Some(confusion) = causes_confusion {
                    for mob in targets.iter() {
                        add_confusion.push((*mob, confusion.turns));
                        if entity == *player_entity {
                            let title = named(names.get(*mob), given_names.get(*mob));
                            let item_name = names.get(useitem.item).unwrap();
                            gamelog.entries.push(format!(
                                "You use {} on {}, confusing them.",
                                item_name.name, title
                            ));
                        }
                        if let Some(pos) = positions.get(*mob) {
                            particle_builder.request(pos.x, pos.y, RGB::named(MAGENTA), RGB::named(BLACK), to_cp437('?'), 200.0)
                        }
                    }
                }
            }
            for mob in add_confusion.iter() {
                confused
                    .insert(mob.0, Confusion { turns: mob.1 })
                    .expect("Unable to insert status");
            }

            let consumable = consumables.get(useitem.item);
            if consumable.is_some() {
                entities.delete(useitem.item).expect("Delete failed")
            }
        }

        wants_drink.clear();
    }
}

pub struct ItemDropSystem {}

impl<'a> System<'a> for ItemDropSystem {
    type SystemData = (
        ReadExpect<'a, Entity>,
        WriteExpect<'a, GameLog>,
        Entities<'a>,
        WriteStorage<'a, WantsToDropItem>,
        ReadStorage<'a, Name>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, InBackpack>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            player_entity,
            mut gamelog,
            entities,
            mut wants_drop,
            names,
            mut positions,
            mut backpack,
        ) = data;

        for (entity, to_drop) in (&entities, &wants_drop).join() {
            let dropper_pos = {
                let dropped_pos = positions.get(entity).unwrap();
                Position {
                    x: dropped_pos.x,
                    y: dropped_pos.y,
                }
            };

            positions
                .insert(to_drop.item, dropper_pos)
                .expect("Unable to insert position");
            backpack.remove(to_drop.item);

            if entity == *player_entity {
                gamelog.entries.push(format!(
                    "You drop the {}.",
                    names.get(to_drop.item).unwrap().name
                ));
            }
        }

        wants_drop.clear();
    }
}

pub struct ItemRemoveSystem {}

impl<'a> System<'a> for ItemRemoveSystem {
    #[allow(clippy::type_complexity)]
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, WantsToRemoveItem>,
        WriteStorage<'a, Equipped>,
        WriteStorage<'a, InBackpack>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (entities, mut wants_remove, mut equipped, mut backpack) = data;

        for (entity, to_remove) in (&entities, &wants_remove).join() {
            equipped.remove(to_remove.item);
            backpack
                .insert(to_remove.item, InBackpack { owner: entity })
                .expect("Unable to insert backpack");
        }

        wants_remove.clear();
    }
}
