use std::borrow::Borrow;

use bracket_lib::{prelude::console, random::RandomNumberGenerator};
use kira::{
    instance::{InstanceSettings, StopInstanceSettings},
    parameter::tween::Tween,
};
use specs::prelude::*;

use crate::{audio::{DesireMusic, Music, SoundResource}, components::Position, gamelog::GameLog, map::Map};
use crate::{
    components::{named, CombatStats, GivenName, Name, Player, SufferDamage},
    RunState,
};

pub struct DamageSystem {}

impl<'a> System<'a> for DamageSystem {
    type SystemData = (
        WriteStorage<'a, CombatStats>,
        WriteStorage<'a, SufferDamage>,
        ReadStorage<'a, Position>,
        WriteExpect<'a, Map>,
        Entities<'a>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (mut stats, mut damage, positions, mut map, entities) = data;

        for (entity, mut stats, damage) in (&entities, &mut stats, &damage).join() {
            stats.hp -= damage.amount.iter().sum::<i32>();
            let pos = positions.get(entity);
            if let Some(pos) = pos {
                let idx = map.xy_idx(pos.x, pos.y);
                map.bloodstains.insert(idx);
            }
        }

        damage.clear();
    }
}

pub fn delete_the_dead(ecs: &mut World) {
    let mut dead: Vec<Entity> = Vec::new();

    {
        let combat_stats = ecs.read_storage::<CombatStats>();
        let players = ecs.read_storage::<Player>();
        let given_names = ecs.read_storage::<GivenName>();
        let names = ecs.read_storage::<Name>();
        let mut sounds = ecs.write_resource::<SoundResource>();
        let mut rng = ecs.write_resource::<RandomNumberGenerator>();
        let entities = ecs.entities();
        let mut log = ecs.write_resource::<GameLog>();
        for (entity, stats) in (&entities, &combat_stats).join() {
            if stats.hp < 1 {
                let player = players.get(entity);
                match player {
                    None => {
                        let title = named(names.get(entity), given_names.get(entity));
                        log.entries.push(format!("{} is dead", title));
                        dead.push(entity);
                        match sounds.play_sound(death_sound(&mut rng), InstanceSettings::default())
                        {
                            Ok(_) => {}
                            Err(e) => console::log(format!("Unable to play sound: {}", e)),
                        }
                    }
                    Some(_) => {
                        let mut desire_music = ecs.write_resource::<DesireMusic>();
                        desire_music.music = Some(Music::GameOver);
                        let mut runstate = ecs.write_resource::<RunState>();
                        *runstate = RunState::GameOver;
                    }
                }
            }
        }
    }

    for victim in dead {
        ecs.delete_entity(victim).expect("Unable to delete victim");
    }
}

fn death_sound(rng: &mut RandomNumberGenerator) -> &str {
    let roll = rng.roll_dice(1, 2);
    match roll {
        1 => "assets/audio/splat_1.ogg",
        2 => "assets/audio/splat_2.ogg",
        _ => "assets/audio/splat_1.ogg",
    }
}
