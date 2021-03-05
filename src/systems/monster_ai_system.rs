use bracket_lib::prelude::*;
use kira::instance::InstanceSettings;
use specs::prelude::*;

use crate::{RunState, audio::SoundResource, components::{Confusion, EntityMoved, Monster, MonsterType, Position, Viewshed, WantsToMelee}, map::Map};

use super::ParticleBuilder;

pub struct MonsterAI {}

impl<'a> System<'a> for MonsterAI {
    #[allow(clippy::type_complexity)]
    type SystemData = (
        WriteExpect<'a, Map>,
        ReadExpect<'a, Point>,
        ReadExpect<'a, Entity>,
        ReadExpect<'a, RunState>,
        Entities<'a>,
        WriteStorage<'a, Viewshed>,
        WriteStorage<'a, Monster>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, WantsToMelee>,
        WriteStorage<'a, Confusion>,
        WriteExpect<'a, ParticleBuilder>,
        WriteStorage<'a, EntityMoved>,
        WriteExpect<'a, SoundResource>,
        WriteExpect<'a, RandomNumberGenerator>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            mut map,
            player_pos,
            player_entity,
            runstate,
            entities,
            mut viewshed,
            mut monster,
            mut position,
            mut wants_to_melee,
            mut confused,
            mut particle_builder,
            mut entity_moved,
            mut sounds,
            mut rng,
        ) = data;

        if *runstate != RunState::MonsterTurn {
            return;
        }

        for (entity, mut viewshed, mut monster, mut pos) in
            (&entities, &mut viewshed, &mut monster, &mut position).join()
        {
            let mut can_act = true;

            let is_confused = confused.get_mut(entity);
            if let Some(i_am_confused) = is_confused {
                i_am_confused.turns -= 1;
                if i_am_confused.turns < 1 {
                    confused.remove(entity);
                }
                can_act = false;
                particle_builder.request(
                    pos.x,
                    pos.y,
                    RGB::named(MAGENTA),
                    RGB::named(BLACK),
                    to_cp437('?'),
                    200.0,
                );
            }

            if can_act {
                let distance =
                    DistanceAlg::Pythagoras.distance2d(Point::new(pos.x, pos.y), *player_pos);

                if distance < 1.5 {
                    wants_to_melee
                        .insert(
                            entity,
                            WantsToMelee {
                                target: *player_entity,
                            },
                        )
                        .expect("Unable to insert attack");
                    return;
                } else if viewshed.visible_tiles.contains(&*player_pos) {
                    let path = a_star_search(
                        map.xy_idx(pos.x, pos.y) as i32,
                        map.xy_idx(player_pos.x, player_pos.y) as i32,
                        &mut *map,
                    );
                    if path.success && path.steps.len() > 1 {
                        let mut idx = map.xy_idx(pos.x, pos.y);
                        map.blocked[idx] = false;
                        pos.x = path.steps[1] as i32 % map.width;
                        pos.y = path.steps[1] as i32 / map.width;
                        idx = map.xy_idx(pos.x, pos.y);
                        map.blocked[idx] = true;
                        viewshed.dirty = true;
                        entity_moved
                            .insert(entity, EntityMoved {})
                            .expect("Unable to insert marker");
                    }

                    if !monster.seen_player {
                        monster.seen_player = true;
                        match sounds.play_sound(
                            monster_noise(&mut rng, monster.monster_type),
                            InstanceSettings::default(),
                        ) {
                            Ok(_) => {}
                            Err(e) => console::log(format!("Unable to play sound: {}", e)),
                        }
                    }
                }
            }
        }
    }
}

fn monster_noise(rng: &mut RandomNumberGenerator, monster_type: MonsterType) -> &str {
    // TODO more noises
    match monster_type {
        MonsterType::Orc => "assets/audio/orc_1.ogg",
        MonsterType::Goblin => "assets/audio/goblin_1.ogg",
    }
}
