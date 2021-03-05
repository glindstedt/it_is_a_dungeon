use bracket_lib::prelude::*;
use kira::instance::InstanceSettings;
use specs::prelude::*;

use crate::{audio::SoundResource, components::{EntityMoved, EntryTrigger, GivenName, Hidden, InflictsDamage, Name, Position, SingleActivation, SufferDamage, named}, gamelog::GameLog, map::Map};

use super::ParticleBuilder;

pub struct TriggerSystem {}

impl<'a> System<'a> for TriggerSystem {
    type SystemData = (
        ReadExpect<'a, Map>,
        WriteStorage<'a, EntityMoved>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, EntryTrigger>,
        WriteStorage<'a, Hidden>,
        ReadStorage<'a, Name>,
        ReadStorage<'a, GivenName>,
        ReadStorage<'a, InflictsDamage>,
        WriteStorage<'a, SufferDamage>,
        WriteExpect<'a, ParticleBuilder>,
        ReadStorage<'a, SingleActivation>,
        Entities<'a>,
        WriteExpect<'a, GameLog>,
        WriteExpect<'a, SoundResource>,
        WriteExpect<'a, RandomNumberGenerator>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (map, mut entity_moved, position, entry_trigger, mut hidden, names, given_names, inflicts_damage, mut suffer_damage, mut particle_builder, single_activation, entities, mut log, mut sounds, mut rng) =
            data;

        let mut remove_entities: Vec<Entity> = Vec::new();
        for (entity, mut _entity_moved, pos) in (&entities, &mut entity_moved, &position).join() {
            let idx = map.xy_idx(pos.x, pos.y);
            for entity_id in map.tile_content[idx].iter() {
                // Don't trigger yourself
                if entity != *entity_id {
                    if entry_trigger.get(*entity_id).is_some() {
                        if let Some(trigger_name) = names.get(*entity_id) {
                            log.entries.push(format!("{} triggers!", trigger_name.name));
                        }

                        // Do damage, if any
                        if let Some(damage) = inflicts_damage.get(*entity_id) {
                            particle_builder.request(pos.x, pos.y, RGB::named(ORANGE), RGB::named(BLACK), to_cp437('‼'), 200.0);
                            SufferDamage::new_damage(&mut suffer_damage, entity, damage.damage);

                            let fool_title = named(names.get(entity), given_names.get(*entity_id));
                            log.entries.push(format!("{} suffers {} damage.", fool_title, damage.damage));

                            match sounds.play_sound(
                                trap_sound(&mut rng),
                                InstanceSettings::default(),
                            ) {
                                Ok(_) => {}
                                Err(e) => console::log(format!("Unable to play sound: {}", e)),
                            }
                        }

                        if single_activation.get(*entity_id).is_some() {
                            remove_entities.push(*entity_id);
                        }

                        hidden.remove(*entity_id); // The trap is no longer hidden
                    }
                }
            }
        }

        for trigger in remove_entities.iter() {
            entities.delete(*trigger).expect("Unable to delete trigger");
        }

        // Remove all entity movement markers
        entity_moved.clear();
    }
}

fn trap_sound(rng: &mut RandomNumberGenerator) -> &str {
    let roll = rng.roll_dice(1, 2);
    match roll {
        1 => "assets/audio/splat_1.ogg",
        2 => "assets/audio/splat_2.ogg",
        _ => "assets/audio/splat_1.ogg",
    }
}
