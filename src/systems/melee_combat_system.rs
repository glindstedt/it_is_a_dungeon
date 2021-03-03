use specs::prelude::*;

use crate::components::{CombatStats, GivenName, Name, SufferDamage, WantsToMelee};
use crate::gamelog::GameLog;

pub struct MeleeCombatSystem {}

impl<'a> System<'a> for MeleeCombatSystem {
    type SystemData = (
        Entities<'a>,
        WriteExpect<'a, GameLog>,
        WriteStorage<'a, WantsToMelee>,
        ReadStorage<'a, Name>,
        ReadStorage<'a, GivenName>,
        ReadStorage<'a, CombatStats>,
        WriteStorage<'a, SufferDamage>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            entities, mut log, mut wants_melee,
            names, given_names, combat_stats, mut inflict_damage
        ) = data;

        for (entity, wants_melee, name, stats) in
            (&entities, &wants_melee, &names, &combat_stats).join()
        {
            let dealer_given_name = given_names.get(entity);
            let dealer_title = match dealer_given_name {
                Some(given_name) => format!("{} the {}", given_name.name, name.name),
                None => name.name.clone(),
            };

            if stats.hp > 0 {
                let target_stats = combat_stats.get(wants_melee.target).unwrap();
                if target_stats.hp > 0 {
                    let target_name = names.get(wants_melee.target).unwrap();
                    let target_given_name = given_names.get(wants_melee.target);
                    // TODO extract and use everywhere
                    let target_title = match target_given_name {
                        Some(given_name) => format!("{} the {}", given_name.name, target_name.name),
                        None => target_name.name.clone(),
                    };

                    let damage = i32::max(0, stats.power - target_stats.defense);

                    if damage == 0 {
                        log.entries.push(format!(
                            "{} is unable to hurt {}",
                            &dealer_title, &target_title
                        ));
                    } else {
                        log.entries.push(format!(
                            "{} hits {}, for {} hp.",
                            &dealer_title, &target_title, damage
                        ));
                        SufferDamage::new_damage(&mut inflict_damage, wants_melee.target, damage);
                    }
                }
            }
        }

        wants_melee.clear();
    }
}
