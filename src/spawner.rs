use std::collections::HashMap;

use bracket_lib::prelude::*;
use specs::{
    prelude::*,
    saveload::{MarkedBuilder, SimpleMarker},
};

use crate::{components::{AreaOfEffect, BlocksTile, CombatStats, Confusion, Consumable, GivenName, InflictsDamage, Item, Monster, Name, Player, Position, ProvidesHealing, Ranged, Renderable, SerializeMe, Viewshed}, random_table::RandomTable};

const MAX_MONSTERS: i32 = 4;

pub fn orc(ecs: &mut World, x: i32, y: i32, given_name: &str) {
    monster(ecs, x, y, to_cp437('o'), "Orc", given_name);
}
pub fn goblin(ecs: &mut World, x: i32, y: i32, given_name: &str) {
    monster(ecs, x, y, to_cp437('g'), "Goblin", given_name);
}

fn monster<S: ToString>(ecs: &mut World, x: i32, y: i32, glyph: FontCharType, name: S, given_name: S) {
    ecs.create_entity()
        .marked::<SimpleMarker<SerializeMe>>()
        .with(Position { x, y })
        .with(Renderable {
            glyph,
            fg: RGB::named(RED),
            bg: RGB::named(BLACK),
            render_order: 1,
        })
        .with(Viewshed {
            visible_tiles: Vec::new(),
            range: 8,
            dirty: true,
        })
        .with(Monster {})
        .with(Name {
            name: name.to_string(),
        })
        .with(GivenName {
            name: given_name.to_string(),
        })
        .with(BlocksTile {})
        .with(CombatStats {
            max_hp: 16,
            hp: 16,
            defense: 1,
            power: 4,
        })
        .build();
}

pub fn player(ecs: &mut World, player_x: i32, player_y: i32) -> Entity {
    ecs.create_entity()
        .marked::<SimpleMarker<SerializeMe>>()
        .with(Position {
            x: player_x,
            y: player_y,
        })
        .with(Renderable {
            glyph: to_cp437('@'),
            fg: RGB::named(YELLOW),
            bg: RGB::named(BLACK),
            render_order: 0,
        })
        .with(Player {})
        .with(Viewshed {
            visible_tiles: Vec::new(),
            range: 8,
            dirty: true,
        })
        .with(Name {
            name: "Player".to_string(),
        })
        .with(CombatStats {
            max_hp: 30,
            hp: 30,
            defense: 2,
            power: 5,
        })
        .build()
}

pub fn health_potion(ecs: &mut World, x: i32, y: i32) {
    ecs.create_entity()
        .marked::<SimpleMarker<SerializeMe>>()
        .with(Position { x, y })
        .with(Renderable {
            glyph: to_cp437(';'),
            fg: RGB::named(MAGENTA),
            bg: RGB::named(BLACK),
            render_order: 2,
        })
        .with(Name {
            name: "Health Potion".to_string(),
        })
        .with(Item {})
        .with(Consumable {})
        .with(ProvidesHealing { heal_amount: 8 })
        .build();
}

pub fn magic_missile_scroll(ecs: &mut World, x: i32, y: i32) {
    ecs.create_entity()
        .marked::<SimpleMarker<SerializeMe>>()
        .with(Position { x, y })
        .with(Renderable {
            glyph: to_cp437(')'),
            fg: RGB::named(CYAN),
            bg: RGB::named(BLACK),
            render_order: 2,
        })
        .with(Name {
            name: "Magic Missile Scroll".to_string(),
        })
        .with(Item {})
        .with(Consumable {})
        .with(Ranged { range: 6 })
        .with(InflictsDamage { damage: 6 })
        .build();
}

pub fn fireball_scroll(ecs: &mut World, x: i32, y: i32) {
    ecs.create_entity()
        .marked::<SimpleMarker<SerializeMe>>()
        .with(Position { x, y })
        .with(Renderable {
            glyph: to_cp437(')'),
            fg: RGB::named(ORANGE),
            bg: RGB::named(BLACK),
            render_order: 2,
        })
        .with(Name {
            name: "Fireball Scroll".to_string(),
        })
        .with(Item {})
        .with(Consumable {})
        .with(Ranged { range: 6 })
        .with(InflictsDamage { damage: 20 })
        .with(AreaOfEffect { radius: 3 })
        .build();
}

pub fn confusion_scroll(ecs: &mut World, x: i32, y: i32) {
    ecs.create_entity()
        .marked::<SimpleMarker<SerializeMe>>()
        .with(Position { x, y })
        .with(Renderable {
            glyph: to_cp437(')'),
            fg: RGB::named(PINK),
            bg: RGB::named(BLACK),
            render_order: 2,
        })
        .with(Name {
            name: "Confusion Scroll".to_string(),
        })
        .with(Item {})
        .with(Consumable {})
        .with(Ranged { range: 6 })
        .with(Confusion { turns: 4 })
        .build();
}

fn room_table(map_depth: i32) -> RandomTable {
    RandomTable::new()
        .add("Goblin", 10)
        .add("Orc", 1 + map_depth)
        .add("Health Potion", 7)
        .add("Fireball Scroll", 2 + map_depth)
        .add("Confusion Scroll", 2 + map_depth)
        .add("Magic Missile Scroll", 4)
}

fn name_table() -> RandomTable {
    RandomTable::new()
        .add("Thomas", 1)
        .add("George", 1)
        .add("Jacques", 1)
        .add("Alex", 1)
        .add("Catriona", 1)
        .add("Lisa", 1)
        .add("Torbjörn", 1)
        .add("Steve", 1)
        .add("Karolina", 1)
        .add("Jenny", 1)
        .add("Chrisp", 1)
        .add("Maria", 1)
        .add("Martin", 1)
        .add("Krisi", 1)
        .add("Ines", 1)
        .add("Marko", 1)
        .add("Miles", 1)
        .add("David", 1)
        .add("Tom", 1)
        .add("Tinks", 1)
        .add("Zemì", 1)
        .add("Remi", 1)
        .add("Mo", 1)
}

pub fn spawn_room(ecs: &mut World, room: &crate::rect::Rect, map_depth: i32) {
    use crate::map::MAPWIDTH;

    let spawn_table = room_table(map_depth);
    let name_table = name_table();
    let mut spawn_points: HashMap<usize, String> = HashMap::new();

    {
        let mut rng = ecs.write_resource::<RandomNumberGenerator>();
        let num_spawns = rng.roll_dice(1, MAX_MONSTERS + 3) + (map_depth - 1) - 3;

        for _ in 0..num_spawns {
            let mut added = false;
            let mut tries = 0;
            while !added && tries < 20 {
                let x = (room.x1 + rng.roll_dice(1, i32::abs(room.x2 - room.x1))) as usize;
                let y = (room.y1 + rng.roll_dice(1, i32::abs(room.y2 - room.y1))) as usize;
                // same as Map.xy_idx()
                let idx = (y * MAPWIDTH) + x;
                if !spawn_points.contains_key(&idx) {
                    spawn_points.insert(idx, spawn_table.roll(&mut rng));
                    added = true;
                } else {
                    tries += 1;
                }
            }
        }
    }

    for spawn in spawn_points.iter() {
        let x = (*spawn.0 % MAPWIDTH) as i32;
        let y = (*spawn.0 / MAPWIDTH) as i32;

        // TODO the name rng scope thingy is pretty ugly
        match spawn.1.as_ref() {
            "Goblin" => {
                let name = {
                    let mut rng = ecs.write_resource::<RandomNumberGenerator>();
                    name_table.roll(&mut rng)
                };
                goblin(ecs, x, y, name.as_str())
            },
            "Orc" => {
                let name = {
                    let mut rng = ecs.write_resource::<RandomNumberGenerator>();
                    name_table.roll(&mut rng)
                };
                orc(ecs, x, y, name.as_str())
            },
            "Health Potion" => health_potion(ecs, x, y),
            "Fireball Scroll" => fireball_scroll(ecs, x, y),
            "Confusion Scroll" => confusion_scroll(ecs, x, y),
            "Magic Missile Scroll" => magic_missile_scroll(ecs, x, y),
            _ => {}
        }
    }
}