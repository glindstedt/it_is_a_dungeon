use audio::SoundResource;
use bracket_lib::prelude::*;
use gui::MainMenuSelection;
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
    ShowRemoveItem,
    ShowTargeting {
        range: i32,
        item: Entity,
    },
    MagicMapReveal {
        row: i32,
        animation: Entity,
    },
    MainMenu {
        menu_selection: gui::MainMenuSelection,
    },
    PreLoading,
    Loading,
    SaveGame,
    NextLevel,
    GameOver,
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

        let mut item_remove = ItemRemoveSystem {};
        item_remove.run_now(&self.ecs);

        let mut particles = ParticleSpawnSystem {};
        particles.run_now(&self.ecs);

        let mut hunger = HungerSystem {};
        hunger.run_now(&self.ecs);

        self.ecs.maintain();
    }

    fn entities_to_remove_on_level_change(&mut self) -> Vec<Entity> {
        let entities = self.ecs.entities();
        let player = self.ecs.read_storage::<Player>();
        let backpack = self.ecs.read_storage::<InBackpack>();
        let equipped = self.ecs.read_storage::<Equipped>();
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
            let eq = equipped.get(entity);
            if let Some(eq) = eq {
                if eq.owner == *player_entity {
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

    fn game_over_cleanup(&mut self) {
        // Delete everything
        let mut to_delete = Vec::new();
        for e in self.ecs.entities().join() {
            to_delete.push(e);
        }
        for del in to_delete.iter() {
            self.ecs.delete_entity(*del).expect("Deletion failed");
        }

        // Build a new map and place the player
        let worldmap;
        {
            let mut worldmap_resource = self.ecs.write_resource::<Map>();
            *worldmap_resource = Map::new_map_rooms_and_corridors(1);
            worldmap = worldmap_resource.clone();
        }

        // Spawn bad guys
        for room in worldmap.rooms.iter().skip(1) {
            spawner::spawn_room(&mut self.ecs, room, 1);
        }

        // Place the player and update resources
        let (player_x, player_y) = worldmap.rooms[0].center();
        let player_entity = spawner::player(&mut self.ecs, player_x, player_y);
        let mut player_position = self.ecs.write_resource::<Point>();
        *player_position = Point::new(player_x, player_y);
        let mut position_components = self.ecs.write_storage::<Position>();
        let mut player_entity_writer = self.ecs.write_resource::<Entity>();
        *player_entity_writer = player_entity;
        let player_pos_comp = position_components.get_mut(player_entity);
        if let Some(player_pos_comp) = player_pos_comp {
            player_pos_comp.x = player_x;
            player_pos_comp.y = player_y;
        }

        // Mark the player's visibility as dirty
        let mut viewshed_components = self.ecs.write_storage::<Viewshed>();
        let vs = viewshed_components.get_mut(player_entity);
        if let Some(vs) = vs {
            vs.dirty = true;
        }
    }

    // TODO remove duplication in new_game and game_over
    fn new_game(&mut self) {
        // Clean up old stuff, if present
        let entities = {
            let mut entities = Vec::new();

            let data = (
                &self.ecs.entities(),
                &self.ecs.read_storage::<SimpleMarker<SerializeMe>>(),
            );
            for (entity, _) in data.join() {
                entities.push(entity);
            }
            entities
        };

        match self.ecs.delete_entities(entities.as_slice()) {
            Ok(_) => {}
            Err(e) => bracket_lib::terminal::console::log(format!("{:?}", e)),
        }

        // Set up new stuff
        let map = Map::new_map_rooms_and_corridors(1);
        for room in map.rooms.iter().skip(1) {
            spawner::spawn_room(&mut self.ecs, room, map.depth);
        }

        let (player_x, player_y) = map.rooms[0].center();

        let player_entity = spawner::player(&mut self.ecs, player_x, player_y);

        self.ecs.insert(Point::new(player_x, player_y));
        self.ecs.insert(player_entity);
        self.ecs.insert(map);
        self.ecs.insert(gamelog::GameLog {
            entries: vec!["Welcome to the deep".to_string()],
        });
    }
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        ctx.cls();
        cull_dead_particles(&mut self.ecs, ctx);

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
            // TODO refactor so there's no duplication between this and GameOver
            RunState::PostRun => {
                // Stop audio loop
                let mut sound_resource = self.ecs.fetch_mut::<SoundResource>();
                sound_resource.stop_all_sounds();
                new_runstate = RunState::MainMenu {
                    menu_selection: gui::MainMenuSelection::LoadGame,
                };
            }
            RunState::GameOver => {
                let result = gui::game_over(ctx);
                match result {
                    gui::GameOverResult::NoSelection => {}
                    gui::GameOverResult::QuitToMenu => {
                        {
                            // Stop audio loop
                            let mut sound_resource = self.ecs.fetch_mut::<SoundResource>();
                            sound_resource.stop_all_sounds();
                        }
                        self.game_over_cleanup();
                        new_runstate = RunState::MainMenu {
                            menu_selection: gui::MainMenuSelection::NewGame,
                        }
                    }
                }
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
                match *self.ecs.fetch::<RunState>() {
                    RunState::MagicMapReveal { row, animation } => {
                        new_runstate = RunState::MagicMapReveal { row, animation }
                    }
                    _ => new_runstate = RunState::MonsterTurn,
                }
            }
            RunState::MagicMapReveal { row, animation } => {
                let mut map = self.ecs.fetch_mut::<Map>();
                for x in 0..map.width {
                    let idx = map.xy_idx(x as i32, row);
                    map.revealed_tiles[idx] = true;
                }
                if row == map.height - 1 {
                    // Cleanup animation
                    self.ecs.entities().delete(animation).unwrap();
                    new_runstate = RunState::MonsterTurn;
                } else {
                    // Animate
                    let mut animations = self.ecs.write_storage::<Animation>();
                    let next_row = if let Some(anim) = animations.get_mut(animation) {
                        anim.elapsed_ms += ctx.frame_time_ms;
                        let ms_per_row = anim.duration_ms / map.height as f32;
                        if anim.elapsed_ms / ms_per_row > row as f32 {
                            row + 1
                        } else {
                            row
                        }
                    } else {
                        row + 1
                    };
                    new_runstate = RunState::MagicMapReveal {
                        row: next_row,
                        animation,
                    };
                }
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
            RunState::ShowRemoveItem => {
                let result = gui::remove_item_menu(self, ctx);
                match result.0 {
                    gui::ItemMenuResult::Cancel => new_runstate = RunState::AwaitingInput,
                    gui::ItemMenuResult::NoResponse => {}
                    gui::ItemMenuResult::Selected => {
                        let item_entity = result.1.unwrap();
                        let mut intent = self.ecs.write_storage::<WantsToRemoveItem>();
                        intent
                            .insert(
                                *self.ecs.fetch::<Entity>(),
                                WantsToRemoveItem { item: item_entity },
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
                            self.new_game();

                            new_runstate = RunState::PreLoading
                        }
                        gui::MainMenuSelection::LoadGame => {
                            match systems::load_game(&mut self.ecs) {
                                Ok(_) => {}
                                Err(e) => bracket_lib::terminal::console::log(format!("{:?}", e)),
                            }
                            new_runstate = RunState::PreLoading;
                            match systems::delete_save() {
                                Ok(_) => {}
                                Err(e) => bracket_lib::terminal::console::log(format!("{:?}", e)),
                            }
                        }
                        gui::MainMenuSelection::Quit => ctx.quit(),
                    },
                }
            }
            // Start all loading processes
            RunState::PreLoading => {
                let mut sound_resource = self.ecs.fetch_mut::<SoundResource>();

                // Load audio from assets if not done previously
                let url = "assets/audio/gr1.ogg";
                sound_resource.load_audio(url);

                new_runstate = RunState::Loading;
            }
            // Wait until all loading processes have finished
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

                    new_runstate = RunState::PreRun;
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

fn main() -> BError {
    let mut context = BTermBuilder::simple80x50()
        .with_title("It is a dungeon")
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
    state.ecs.register::<WantsToRemoveItem>();
    state.ecs.register::<WantsToUseItem>();
    state.ecs.register::<Consumable>();
    state.ecs.register::<ProvidesHealing>();
    state.ecs.register::<Ranged>();
    state.ecs.register::<InflictsDamage>();
    state.ecs.register::<AreaOfEffect>();
    state.ecs.register::<Confusion>();
    state.ecs.register::<Equipable>();
    state.ecs.register::<Equipped>();
    state.ecs.register::<MeleePowerBonus>();
    state.ecs.register::<DefenceBonus>();
    state.ecs.register::<ParticleLifetime>();
    state.ecs.register::<HungerClock>();
    state.ecs.register::<ProvidesFood>();
    state.ecs.register::<MagicMapper>();
    state.ecs.register::<Animation>();
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
    state.ecs.insert(ParticleBuilder::default());

    state.new_game();

    main_loop(context, state)
}
