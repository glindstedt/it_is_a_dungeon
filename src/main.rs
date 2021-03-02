use bracket_lib::prelude::*;
use specs::{
    prelude::*,
    saveload::{SimpleMarker, SimpleMarkerAllocator},
};

mod components;
mod console;
mod gamelog;
mod gui;
mod map;
mod player;
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
    SaveGame,
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
                            new_runstate = RunState::PreRun
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
            RunState::SaveGame => {
                match systems::save_game(&mut self.ecs) {
                    Ok(_) => {}
                    Err(e) => bracket_lib::terminal::console::log(format!("{:?}", e)),
                }
                new_runstate = RunState::MainMenu {
                    menu_selection: gui::MainMenuSelection::LoadGame,
                }
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
    let map = Map::new_map_rooms_and_corridors();
    for room in map.rooms.iter().skip(1) {
        spawner::spawn_room(ecs, room);
    }

    let (player_x, player_y) = map.rooms[0].center();

    let player_entity = spawner::player(ecs, player_x, player_y);

    ecs.insert(Point::new(player_x, player_y));
    ecs.insert(player_entity);
    ecs.insert(map);
    ecs.insert(gamelog::GameLog {
        entries: vec!["Welcome to Roguelike".to_string()],
    });
}

fn main() -> BError {
    let mut context = BTermBuilder::simple80x50()
        .with_title("Roguelike Tutorial")
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

    new_game(&mut state.ecs);

    main_loop(context, state)
}
