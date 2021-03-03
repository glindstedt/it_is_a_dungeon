use audio::SoundResource;
use bracket_lib::prelude::*;
use kira::{
    instance::InstanceSettings,
    manager::{AudioManager, AudioManagerSettings},
    sound::SoundSettings,
    Frame,
};
use specs::{
    prelude::*,
    saveload::{SimpleMarker, SimpleMarkerAllocator},
};

mod audio;
mod components;
mod console;
mod gamelog;
mod gui;
mod map;
mod player;
mod random_table;
mod rect;
mod spawner;
mod systems;

use crate::{
    components::*,
    console::console_input,
    map::{draw_map, Map},
    player::player_input,
    systems::*,
};

#[derive(PartialEq, Copy, Clone)]
pub enum RunState {
    AwaitingInput,
    Console,
    PreRun,
    PostRun,
    PlayerTurn,
    MonsterTurn,
    ShowInventory,
    ShowDropItem,
    ShowTargeting {
        range: i32,
        item: Entity,
    },
    MainMenu {
        menu_selection: gui::MainMenuSelection,
    },
    Loading,
    SaveGame,
    NextLevel,
}

pub struct State {
    ecs: World,
}

impl State {
    fn run_systems(&mut self) {
        let mut vis = VisibilitySystem {};
        vis.run_now(&self.ecs);

        let mut mob = MonsterAI {};
        mob.run_now(&self.ecs);

        let mut mapindex = MapIndexingSystem {};
        mapindex.run_now(&self.ecs);

        let mut melee = MeleeCombatSystem {};
        melee.run_now(&self.ecs);

        let mut damage = DamageSystem {};
        damage.run_now(&self.ecs);

        let mut pickup = ItemCollectionSystem {};
        pickup.run_now(&self.ecs);

        let mut potions = ItemUseSystem {};
        potions.run_now(&self.ecs);

        let mut drop_items = ItemDropSystem {};
        drop_items.run_now(&self.ecs);

        self.ecs.maintain();
    }

    fn entities_to_remove_on_level_change(&mut self) -> Vec<Entity> {
        let entities = self.ecs.entities();
        let player = self.ecs.read_storage::<Player>();
        let backpack = self.ecs.read_storage::<InBackpack>();
        let player_entity = self.ecs.fetch::<Entity>();

        let mut to_delete: Vec<Entity> = Vec::new();
        for entity in entities.join() {
            let mut should_delete = true;

            // Don't delete the player
            let p = player.get(entity);
            if let Some(_p) = p {
                should_delete = false;
            }

            // Don't delete the player's equipment
            let bp = backpack.get(entity);
            if let Some(bp) = bp {
                if bp.owner == *player_entity {
                    should_delete = false;
                }
            }

            if should_delete {
                to_delete.push(entity);
            }
        }

        to_delete
    }

    fn goto_next_level(&mut self) {
        // Delete entities that aren't the player or his/her equipment
        let to_delete = self.entities_to_remove_on_level_change();
        for target in to_delete {
            self.ecs
                .delete_entity(target)
                .expect("Unable to delete entity");
        }

        // Build a new map and place the player
        let map = {
            let mut map = self.ecs.write_resource::<Map>();
            *map = Map::new_map_rooms_and_corridors(map.depth + 1);
            map.clone()
        };

        // Spawn bad guys
        for room in map.rooms.iter().skip(1) {
            spawner::spawn_room(&mut self.ecs, room, map.depth);
        }

        // Place the player and update resources
        let (player_x, player_y) = map.rooms[0].center();
        let mut player_position = self.ecs.write_resource::<Point>();
        *player_position = Point::new(player_x, player_y);
        let mut position_components = self.ecs.write_storage::<Position>();
        let player_entity = self.ecs.fetch::<Entity>();
        let player_pos_comp = position_components.get_mut(*player_entity);
        if let Some(player_pos_comp) = player_pos_comp {
            player_pos_comp.x = player_x;
            player_pos_comp.y = player_y;
        }

        // Mark the player's visibility as dirty
        let mut viewshed_components = self.ecs.write_storage::<Viewshed>();
        let vs = viewshed_components.get_mut(*player_entity);
        if let Some(vs) = vs {
            vs.dirty = true;
        }

        // Notify the player and give them some health
        let mut gamelog = self.ecs.fetch_mut::<gamelog::GameLog>();
        gamelog
            .entries
            .push("You descend to the next level, and take a moment to heal.".to_string());
        let mut player_health_store = self.ecs.write_storage::<CombatStats>();
        let player_health = player_health_store.get_mut(*player_entity);
        if let Some(player_health) = player_health {
            player_health.hp = i32::max(player_health.hp, player_health.max_hp / 2);
        }
    }
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        ctx.cls();

        {}

        let mut new_runstate = {
            let runstate = self.ecs.fetch::<RunState>();
            *runstate
        };

        // To draw or not to draw???
        match new_runstate {
            RunState::MainMenu { .. } => {}
            // TODO loading
            _ => {
                draw_map(&self.ecs, ctx);

                let positions = self.ecs.read_storage::<Position>();
                let renderables = self.ecs.read_storage::<Renderable>();
                let map = self.ecs.fetch::<Map>();

                // Render monsters and objects
                let mut data = (&positions, &renderables).join().collect::<Vec<_>>();
                data.sort_by(|&a, &b| b.1.render_order.cmp(&a.1.render_order));
                for (pos, render) in data.iter() {
                    let idx = map.xy_idx(pos.x, pos.y);
                    if map.fog_off || map.visible_tiles[idx] {
                        ctx.set(pos.x, pos.y, render.fg, render.bg, render.glyph);
                    }
                }

                gui::draw_ui(&self.ecs, ctx);
            }
        }

        match new_runstate {
            // Initial
            RunState::PreRun => {
                self.run_systems();

                new_runstate = RunState::AwaitingInput;
            }
            RunState::PostRun => {
                // Stop audio loop
                let mut sound_resource = self.ecs.fetch_mut::<SoundResource>();
                sound_resource.stop_all_sounds();
                new_runstate = RunState::MainMenu {
                    menu_selection: gui::MainMenuSelection::LoadGame,
                };
            }
            // Debug console
            RunState::Console => {
                new_runstate = console_input(self, ctx);
            }
            // Game loop
            RunState::AwaitingInput => {
                new_runstate = player_input(self, ctx);
            }
            RunState::PlayerTurn => {
                self.run_systems();
                new_runstate = RunState::MonsterTurn;
            }
            RunState::MonsterTurn => {
                self.run_systems();
                new_runstate = RunState::AwaitingInput;
            }
            RunState::ShowTargeting { range, item } => {
                let result = gui::ranged_target(self, ctx, range);
                match result.0 {
                    gui::ItemMenuResult::Cancel => new_runstate = RunState::AwaitingInput,
                    gui::ItemMenuResult::NoResponse => {}
                    gui::ItemMenuResult::Selected => {
                        let mut intent = self.ecs.write_storage::<WantsToUseItem>();
                        intent
                            .insert(
                                *self.ecs.fetch::<Entity>(),
                                WantsToUseItem {
                                    item,
                                    target: result.1,
                                },
                            )
                            .expect("Unable to insert intent");
                        new_runstate = RunState::PlayerTurn;
                    }
                }
            }
            RunState::ShowInventory => {
                let result = gui::show_inventory(self, ctx);
                match result.0 {
                    gui::ItemMenuResult::Cancel => new_runstate = RunState::AwaitingInput,
                    gui::ItemMenuResult::NoResponse => {}
                    gui::ItemMenuResult::Selected => {
                        let item_entity = result.1.unwrap();
                        let ranged = self.ecs.read_storage::<Ranged>();
                        let is_item_ranged = ranged.get(item_entity);
                        if let Some(is_item_ranged) = is_item_ranged {
                            new_runstate = RunState::ShowTargeting {
                                range: is_item_ranged.range,
                                item: item_entity,
                            };
                        } else {
                            let mut intent = self.ecs.write_storage::<WantsToUseItem>();
                            intent
                                .insert(
                                    *self.ecs.fetch::<Entity>(),
                                    WantsToUseItem {
                                        item: item_entity,
                                        target: None,
                                    },
                                )
                                .expect("Unable to insert intent");
                            new_runstate = RunState::PlayerTurn;
                        }
                    }
                }
            }
            RunState::ShowDropItem => {
                let result = gui::drop_item_menu(self, ctx);
                match result.0 {
                    gui::ItemMenuResult::Cancel => new_runstate = RunState::AwaitingInput,
                    gui::ItemMenuResult::NoResponse => {}
                    gui::ItemMenuResult::Selected => {
                        let item_entity = result.1.unwrap();
                        let mut intent = self.ecs.write_storage::<WantsToDropItem>();
                        intent
                            .insert(
                                *self.ecs.fetch::<Entity>(),
                                WantsToDropItem { item: item_entity },
                            )
                            .expect("Unable to insert intent");
                        new_runstate = RunState::PlayerTurn;
                    }
                }
            }

            RunState::MainMenu { .. } => {
                let result = gui::main_menu(self, ctx);
                match result {
                    gui::MainMenuResult::NoSelection { selected } => {
                        new_runstate = RunState::MainMenu {
                            menu_selection: selected,
                        }
                    }
                    gui::MainMenuResult::Selected { selected } => match selected {
                        gui::MainMenuSelection::NewGame => {
                            new_game(&mut self.ecs);

                            // Load audio
                            let url = "assets/audio/gr1.ogg";
                            let mut sound_resource = self.ecs.fetch_mut::<SoundResource>();
                            sound_resource.load_audio(url);

                            new_runstate = RunState::Loading
                        }
                        gui::MainMenuSelection::LoadGame => {
                            match systems::load_game(&mut self.ecs) {
                                Ok(_) => {}
                                Err(e) => bracket_lib::terminal::console::log(format!("{:?}", e)),
                            }
                            new_runstate = RunState::AwaitingInput;
                            match systems::delete_save() {
                                Ok(_) => {}
                                Err(e) => bracket_lib::terminal::console::log(format!("{:?}", e)),
                            }
                        }
                        gui::MainMenuSelection::Quit => ctx.quit(),
                    },
                }
            }
            RunState::Loading => {
                let mut sound_resource = self.ecs.fetch_mut::<SoundResource>();
                let mut audio_manager = self.ecs.fetch_mut::<AudioManager>();
                if !sound_resource.finished_loading() {
                    bracket_lib::terminal::console::log("Handling load queue...");
                    // TODO handle error
                    sound_resource
                        .handle_load_queue(&mut audio_manager)
                        .unwrap();
                } else {
                    bracket_lib::terminal::console::log("Loaded!");

                    // Start audio loop
                    // let mut sound_resource = self.ecs.fetch_mut::<SoundResource>();
                    // TODO handle error
                    sound_resource
                        .play_sound(
                            "assets/audio/gr1.ogg",
                            InstanceSettings::default().loop_start(0f64),
                        )
                        .unwrap();

                    new_runstate = RunState::PreRun
                }
            }
            RunState::SaveGame => {
                match systems::save_game(&mut self.ecs) {
                    Ok(_) => {}
                    Err(e) => bracket_lib::terminal::console::log(format!("{:?}", e)),
                }
                new_runstate = RunState::PostRun;
            }
            RunState::NextLevel => {
                self.goto_next_level();
                new_runstate = RunState::PreRun;
            }
        }

        {
            let mut runwriter = self.ecs.write_resource::<RunState>();
            *runwriter = new_runstate;
        }

        delete_the_dead(&mut self.ecs);
    }
}

fn new_game(ecs: &mut World) {
    // Clean up old stuff, if present
    let entities = {
        let mut entities = Vec::new();

        let data = (
            &ecs.entities(),
            &ecs.read_storage::<SimpleMarker<SerializeMe>>(),
        );
        for (entity, _) in data.join() {
            entities.push(entity);
        }
        entities
    };

    match ecs.delete_entities(entities.as_slice()) {
        Ok(_) => {}
        Err(e) => bracket_lib::terminal::console::log(format!("{:?}", e)),
    }

    // Set up new stuff
    let map = Map::new_map_rooms_and_corridors(1);
    for room in map.rooms.iter().skip(1) {
        spawner::spawn_room(ecs, room, map.depth);
    }

    let (player_x, player_y) = map.rooms[0].center();

    let player_entity = spawner::player(ecs, player_x, player_y);

    ecs.insert(Point::new(player_x, player_y));
    ecs.insert(player_entity);
    ecs.insert(map);
    ecs.insert(gamelog::GameLog {
        entries: vec!["Welcome to the deep".to_string()],
    });
}

fn main() -> BError {
    let mut context = BTermBuilder::simple80x50()
        .with_title("It is a game")
        .build()?;
    // context.with_post_scanlines(true);

    let mut state = State { ecs: World::new() };

    // Markers
    state.ecs.register::<SimpleMarker<SerializeMe>>();

    // Components
    state.ecs.register::<Position>();
    state.ecs.register::<Renderable>();
    state.ecs.register::<Player>();
    state.ecs.register::<Viewshed>();
    state.ecs.register::<Name>();
    state.ecs.register::<GivenName>();
    state.ecs.register::<Monster>();
    state.ecs.register::<BlocksTile>();
    state.ecs.register::<CombatStats>();
    state.ecs.register::<WantsToMelee>();
    state.ecs.register::<SufferDamage>();
    state.ecs.register::<Item>();
    state.ecs.register::<InBackpack>();
    state.ecs.register::<WantsToPickupItem>();
    state.ecs.register::<WantsToDropItem>();
    state.ecs.register::<WantsToUseItem>();
    state.ecs.register::<Consumable>();
    state.ecs.register::<ProvidesHealing>();
    state.ecs.register::<Ranged>();
    state.ecs.register::<InflictsDamage>();
    state.ecs.register::<AreaOfEffect>();
    state.ecs.register::<Confusion>();
    state.ecs.register::<SerializationHelper>();

    // Resources
    state
        .ecs
        .insert(SimpleMarkerAllocator::<SerializeMe>::new());
    state.ecs.insert(RandomNumberGenerator::new());
    state.ecs.insert(RunState::MainMenu {
        menu_selection: gui::MainMenuSelection::NewGame,
    });
    state.ecs.insert(console::Console::new());

    let audio_manager =
        AudioManager::new(AudioManagerSettings::default()).expect("Unable to initialize audio");
    state.ecs.insert(audio_manager);
    state.ecs.insert(SoundResource::default());

    new_game(&mut state.ecs);

    main_loop(context, state)
}
