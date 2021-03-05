use bracket_lib::prelude::*;
use kira::instance::InstanceSettings;
use specs::prelude::*;

use crate::{
    audio::SoundResource,
    components::{
        named, CombatStats, DefenceBonus, Equipped, GivenName, HungerClock, HungerState,
        MeleePowerBonus, Name, Position, SufferDamage, WantsToMelee,
    },
};
use crate::{components::MeleeType, gamelog::GameLog};

use super::ParticleBuilder;

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
        ReadStorage<'a, Equipped>,
        ReadStorage<'a, MeleePowerBonus>,
        ReadStorage<'a, DefenceBonus>,
        WriteExpect<'a, ParticleBuilder>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, HungerClock>,
        WriteExpect<'a, SoundResource>,
        WriteExpect<'a, RandomNumberGenerator>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            entities,
            mut log,
            mut wants_melee,
            names,
            given_names,
            combat_stats,
            mut inflict_damage,
            equipped,
            melee_power_bonuses,
            defence_bonuses,
            mut particle_builder,
            positions,
            hunger_clock,
            mut sounds,
            mut rng,
        ) = data;

        let mut melee_type = MeleeType::Blunt;
        let mut melee_was_had = false;
        for (entity, wants_melee, name, stats) in
            (&entities, &wants_melee, &names, &combat_stats).join()
        {
            let dealer_title = named(Some(&name), given_names.get(entity));

            if stats.hp > 0 {
                let mut offensive_bonus = 0;
                for (_, power_bonus, equipped_by) in
                    (&entities, &melee_power_bonuses, &equipped).join()
                {
                    if equipped_by.owner == entity {
                        offensive_bonus += power_bonus.power;
                        melee_type = power_bonus.melee_type;
                    }
                }

                let target_stats = combat_stats.get(wants_melee.target).unwrap();
                if target_stats.hp > 0 {
                    let target_title = named(
                        names.get(wants_melee.target),
                        given_names.get(wants_melee.target),
                    );
                    let mut defensive_bonus = 0;
                    for (_, defence_bonus, equipped_by) in
                        (&entities, &defence_bonuses, &equipped).join()
                    {
                        if equipped_by.owner == wants_melee.target {
                            defensive_bonus += defence_bonus.defence
                        }
                    }
                    if let Some(pos) = positions.get(wants_melee.target) {
                        particle_builder.request(
                            pos.x,
                            pos.y,
                            RGB::named(ORANGE),
                            RGB::named(BLACK),
                            to_cp437('‼'),
                            200.0,
                        );
                    }

                    if let Some(hc) = hunger_clock.get(entity) {
                        if hc.state == HungerState::WellFed {
                            offensive_bonus += 1;
                        }
                    }

                    let damage = i32::max(
                        0,
                        (stats.power + offensive_bonus) - (target_stats.defence + defensive_bonus),
                    );
                    if damage == 0 {
                        log.entries.push(format!(
                            "{} is unable to hurt {}",
                            &dealer_title, &target_title
                        ));
                    } else {
                        log.entries.push(format!(
                            "{} hits {}, for {} hp. ({}(+{})-{}(+{})",
                            &dealer_title,
                            &target_title,
                            damage,
                            stats.power,
                            offensive_bonus,
                            target_stats.defence,
                            defensive_bonus
                        ));
                        melee_was_had = true;
                        SufferDamage::new_damage(&mut inflict_damage, wants_melee.target, damage);
                    }
                }
            }
        }

        // TODO make sound effect system instead
        if melee_was_had {
            match sounds.play_sound(
                melee_sound(&mut rng, melee_type),
                InstanceSettings::default(),
            ) {
                Ok(_) => {}
                Err(e) => console::log(format!("Unable to play sound: {}", e)),
            }
        }

        wants_melee.clear();
    }
}

fn melee_sound(rng: &mut RandomNumberGenerator, melee_type: MeleeType) -> &str {
    let roll = rng.roll_dice(1, 3);

    match melee_type {
        MeleeType::Blunt => match roll {
            1 => "assets/audio/punch_1.ogg",
            2 => "assets/audio/punch_2.ogg",
            3 => "assets/audio/punch_3.ogg",
            _ => "assets/audio/punch_1.ogg",
        },
        MeleeType::Slash => match roll {
            1 => "assets/audio/blade_1.ogg",
            2 => "assets/audio/blade_2.ogg",
            3 => "assets/audio/blade_3.ogg",
            _ => "assets/audio/blade_1.ogg",
        },
    }
}
