use bracket_lib::prelude::*;
use serde::{Deserialize, Serialize};
use specs::{
    error::NoError,
    prelude::*,
    saveload::{ConvertSaveload, Marker},
    Component, ConvertSaveload,
};

mod helpers;
mod intent;

pub use helpers::*;
pub use intent::*;

use crate::audio::Music;

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct Renderable {
    pub glyph: FontCharType,
    pub fg: RGB,
    pub bg: RGB,
    pub render_order: i32,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct Viewshed {
    pub visible_tiles: Vec<Point>,
    pub range: i32,
    pub dirty: bool,
}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct Player {}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct Name {
    pub name: String,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct GivenName {
    pub name: String,
}

#[derive(PartialEq, Copy, Clone, Serialize, Deserialize, Debug)]
pub enum MonsterType {
    Orc,
    Goblin,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct Monster {
    pub monster_type: MonsterType,
    pub seen_player: bool,
}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct BlocksTile {}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct CombatStats {
    pub max_hp: i32,
    pub hp: i32,
    pub defence: i32,
    pub power: i32,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct SufferDamage {
    pub amount: Vec<i32>,
}

impl SufferDamage {
    pub fn new_damage(store: &mut WriteStorage<SufferDamage>, victim: Entity, amount: i32) {
        if let Some(suffering) = store.get_mut(victim) {
            suffering.amount.push(amount);
        } else {
            let dmg = SufferDamage {
                amount: vec![amount],
            };
            store.insert(victim, dmg).expect("Unable to insert damage");
        }
    }
}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct Item {}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct InBackpack {
    pub owner: Entity,
}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct Consumable {}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct ProvidesHealing {
    pub heal_amount: i32,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct Ranged {
    pub range: i32,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct InflictsDamage {
    pub damage: i32,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct AreaOfEffect {
    pub radius: i32,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct Confusion {
    pub turns: i32,
}

#[derive(PartialEq, Copy, Clone, Serialize, Deserialize, Debug)]
pub enum EquipmentSlot {
    Melee,
    Shield,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct Equipable {
    pub slot: EquipmentSlot,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct Equipped {
    pub owner: Entity,
    pub slot: EquipmentSlot,
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
pub enum MeleeType {
    Blunt,
    Slash,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct MeleePowerBonus {
    pub power: i32,
    pub melee_type: MeleeType,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct DefenceBonus {
    pub defence: i32,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct ParticleLifetime {
    pub lifetime_ms: f32,
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
pub enum HungerState {
    WellFed,
    Normal,
    Hungry,
    Starving,
}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct HungerClock {
    pub state: HungerState,
    pub duration: i32,
}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct ProvidesFood {}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct MagicMapper {}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct Animation {
    pub duration_ms: f32,
    pub elapsed_ms: f32,
}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct Hidden {}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct EntryTrigger {}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct EntityMoved {}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct SingleActivation {}

#[derive(Component, ConvertSaveload, Debug, Clone)]
pub struct SerializationHelper {
    pub map: crate::map::Map,
}

pub struct SerializeMe;
