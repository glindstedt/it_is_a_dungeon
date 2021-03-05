use std::{
    fs::{self, File},
    io::{self},
    path::Path,
};

use bracket_lib::prelude::*;
use serde_json::Serializer;
use specs::{
    error::NoError,
    prelude::*,
    saveload::{
        DeserializeComponents, MarkedBuilder, SerializeComponents, SimpleMarker,
        SimpleMarkerAllocator,
    },
    World, WorldExt,
};
use thiserror::Error;

use crate::components::*;

#[derive(Error, Debug)]
pub enum SaveLoadError {
    #[error("error saving or loading game: {source}")]
    Error {
        #[from]
        source: anyhow::Error,
    },
}

macro_rules! serialize_individually {
    ($ecs:expr, $ser:expr, $data:expr, $( $type:ty),*) => {
        $(
        SerializeComponents::<NoError, SimpleMarker<SerializeMe>>::serialize(
            &( $ecs.read_storage::<$type>(), ),
            &$data.0,
            &$data.1,
            &mut $ser,
        )
        .unwrap();
        )*
    };
}

macro_rules! deserialize_individually {
    ($ecs:expr, $de:expr, $data:expr, $( $type:ty),*) => {
        $(
        DeserializeComponents::<NoError, _>::deserialize(
            &mut ( &mut $ecs.write_storage::<$type>(), ),
            &mut $data.0, // entities
            &mut $data.1, // marker
            &mut $data.2, // allocater
            &mut $de,
        )
        .unwrap();
        )*
    };
}

pub fn save_game(ecs: &mut World) -> Result<(), SaveLoadError> {
    console::log("Saving game...");
    let mapcopy = ecs.get_mut::<crate::map::Map>().unwrap().clone();
    let savehelper = ecs
        .create_entity()
        .marked::<SimpleMarker<SerializeMe>>()
        .with(SerializationHelper { map: mapcopy })
        .build();

    {
        let data = (
            ecs.entities(),
            ecs.read_storage::<SimpleMarker<SerializeMe>>(),
        );

        let mut serializer = serde_json::Serializer::new(get_save_writer()?);

        serialize_individually!(
            ecs,
            serializer,
            data,
            Position,
            Renderable,
            Viewshed,
            Player,
            Name,
            GivenName,
            Monster,
            BlocksTile,
            CombatStats,
            SufferDamage,
            Item,
            InBackpack,
            Consumable,
            ProvidesHealing,
            Ranged,
            InflictsDamage,
            AreaOfEffect,
            Confusion,
            Equipable,
            Equipped,
            MeleePowerBonus,
            DefenceBonus,
            ParticleLifetime,
            HungerClock,
            ProvidesFood,
            MagicMapper,
            Animation,
            WantsToMelee,
            WantsToPickupItem,
            WantsToDropItem,
            WantsToRemoveItem,
            WantsToUseItem,
            SerializationHelper
        );

        finalize_serializer(serializer)?;
    }
    ecs.delete_entity(savehelper).expect("Crash on cleanup");
    Ok(())
}

pub fn load_game(ecs: &mut World) -> Result<(), SaveLoadError> {
    // Delete everything
    ecs.delete_all();

    let data = get_save_data()?;
    let mut de = serde_json::Deserializer::from_str(&data);

    {
        let mut d = (
            &mut ecs.entities(),
            &mut ecs.write_storage::<SimpleMarker<SerializeMe>>(),
            &mut ecs.write_resource::<SimpleMarkerAllocator<SerializeMe>>(),
        );
        deserialize_individually!(
            ecs,
            de,
            d,
            Position,
            Renderable,
            Viewshed,
            Player,
            Name,
            GivenName,
            Monster,
            BlocksTile,
            CombatStats,
            SufferDamage,
            Item,
            InBackpack,
            Consumable,
            ProvidesHealing,
            Ranged,
            InflictsDamage,
            AreaOfEffect,
            Confusion,
            Equipable,
            Equipped,
            MeleePowerBonus,
            DefenceBonus,
            ParticleLifetime,
            HungerClock,
            ProvidesFood,
            MagicMapper,
            Animation,
            WantsToMelee,
            WantsToPickupItem,
            WantsToDropItem,
            WantsToRemoveItem,
            WantsToUseItem,
            SerializationHelper
        );
    }

    let mut deleteme: Option<Entity> = None;

    {
        let entities = ecs.entities();
        let helper = ecs.read_storage::<SerializationHelper>();
        let player = ecs.read_storage::<Player>();
        let position = ecs.read_storage::<Position>();
        for (e, h) in (&entities, &helper).join() {
            let mut worldmap = ecs.write_resource::<crate::map::Map>();
            *worldmap = h.map.clone();
            worldmap.tile_content = vec![Vec::new(); crate::map::MAPCOUNT];
            deleteme = Some(e);
        }
        for (e, _p, pos) in (&entities, &player, &position).join() {
            let mut ppos = ecs.write_resource::<Point>();
            *ppos = Point::new(pos.x, pos.y);
            let mut player_resource = ecs.write_resource::<Entity>();
            *player_resource = e;
        }
    }
    ecs.delete_entity(deleteme.unwrap())
        .expect("Unable to delete helper");

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn does_save_exist() -> bool {
    Path::new("./savegame.json").exists()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_save_data() -> anyhow::Result<String> {
    Ok(fs::read_to_string("./savegame.json")?)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn delete_save() -> anyhow::Result<()> {
    if does_save_exist() {
        std::fs::remove_file("./savegame.json")?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_save_writer() -> anyhow::Result<impl io::Write> {
    Ok(File::create("./savegame.json")?)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn finalize_serializer(_ser: Serializer<impl io::Write>) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
use io::BufWriter;

#[cfg(target_arch = "wasm32")]
const SAVEGAME: &'static str = "savegame";

#[cfg(target_arch = "wasm32")]
pub fn does_save_exist() -> bool {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(local_storage)) = window.local_storage() {
            match local_storage.get_item(SAVEGAME) {
                Ok(val) => {
                    return val.is_some();
                }
                Err(e) => {
                    console::log(format!("{:?}", e));
                    return false;
                }
            }
        } else {
            return false;
        }
    }
    false
}

#[cfg(target_arch = "wasm32")]
pub fn get_save_data() -> anyhow::Result<String> {
    let local_storage = web_sys::window()
        .ok_or(anyhow::anyhow!("Unable to get window"))?
        .local_storage();

    if let Ok(Some(local_storage)) = local_storage {
        match local_storage.get_item(SAVEGAME) {
            Ok(Some(save)) => return Ok(save),
            Ok(_) => return Err(anyhow::anyhow!("No save data")),
            Err(e) => {
                console::log(format!("{:?}", e));
                return Err(anyhow::anyhow!("Unable to load save"));
            }
        }
    } else {
        return Err(anyhow::anyhow!("Failed to access local storage"));
    }
}

#[cfg(target_arch = "wasm32")]
pub fn delete_save() -> anyhow::Result<()> {
    let local_storage = web_sys::window()
        .ok_or(anyhow::anyhow!("Unable to get window"))?
        .local_storage();
    //TODO
    if let Ok(Some(local_storage)) = local_storage {
        match local_storage.remove_item(SAVEGAME) {
            Ok(_) => return Ok(()),
            Err(e) => {
                console::log(format!("{:?}", e));
                return Err(anyhow::anyhow!("Unable to delete save"));
            }
        }
    } else {
        return Err(anyhow::anyhow!("Failed to access local storage"));
    }
}

#[cfg(target_arch = "wasm32")]
pub fn get_save_writer() -> anyhow::Result<BufWriter<Vec<u8>>> {
    Ok(BufWriter::new(Vec::new()))
}

#[cfg(target_arch = "wasm32")]
pub fn finalize_serializer(ser: Serializer<BufWriter<Vec<u8>>>) -> anyhow::Result<()> {
    let saved_game = ser.into_inner().into_inner()?;

    let saved_game_json = String::from_utf8(saved_game).map_err(|e| SaveLoadError::Error {
        source: anyhow::anyhow!(e),
    })?;

    // Save to local storage
    let local_storage = web_sys::window()
        .ok_or(anyhow::anyhow!("Unable to get window"))?
        .local_storage();
    if let Ok(Some(local_storage)) = local_storage {
        local_storage
            .set_item(SAVEGAME, saved_game_json.as_str())
            .expect("Failed to set local storage item");
        return Ok(());
    } else {
        return Err(anyhow::anyhow!("Failed to access local storage"));
    }
}
