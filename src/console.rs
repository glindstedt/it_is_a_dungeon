use std::cmp::{max, min};

use bracket_lib::prelude::*;
use specs::WorldExt;
use thiserror::Error;

use crate::{map::Map, spawner, DebugOptions, RunState, State};

#[derive(PartialEq, Debug, Clone)]
pub enum Line {
    Input(String),
    Output(String),
}

impl Line {
    pub fn is_input(&self) -> bool {
        match self {
            Self::Input(_) => true,
            _ => false,
        }
    }
    pub fn is_output(&self) -> bool {
        match self {
            Self::Output(_) => true,
            _ => false,
        }
    }
}

pub struct Console {
    pub height: usize,
    pub history: Vec<Line>,
    pub history_index: Option<usize>,
    pub input_buffer: String,
}

impl Console {
    pub fn new() -> Self {
        Console {
            height: 10,
            history: vec![],
            history_index: None,
            input_buffer: String::new(),
        }
    }

    fn input_history(&self) -> impl Iterator<Item = &Line> {
        self.history.iter().filter(|l| l.is_input()).rev()
    }
}

#[derive(Error, Debug)]
pub enum ConsoleError {
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("invalid argument `{found}`, expected {expected}")]
    InvalidArgument {
        #[source]
        source: anyhow::Error,
        expected: String,
        found: String,
    },
    #[error("insufficient arguments, expected at least {0}")]
    InsufficientArguments(usize),
}

pub fn console_input(gs: &mut State, ctx: &mut BTerm) -> RunState {
    use Line::*;
    use VirtualKeyCode::*;

    let mut next_state = RunState::Console;

    match ctx.key {
        None => {}
        Some(key) => match key {
            Grave | Escape => return RunState::AwaitingInput,
            Back => {
                let mut console = gs.ecs.fetch_mut::<Console>();
                console.input_buffer.pop();
            }
            // History scrolling
            Up => {
                let mut console = gs.ecs.fetch_mut::<Console>();
                let input_history: Vec<Line> = console.input_history().cloned().collect();
                if input_history.len() > 0 {
                    let new_history_index = match console.history_index {
                        None => Some(0),
                        Some(val) => Some(min(input_history.len() - 1, val + 1)),
                    };
                    console.history_index = new_history_index;
                    if let Input(input) = &input_history[console.history_index.unwrap()] {
                        console.input_buffer = input.clone();
                    }
                }
            }
            Down => {
                let mut console = gs.ecs.fetch_mut::<Console>();
                // Update index
                let new_history_index = if let Some(val) = console.history_index {
                    if val == 0 {
                        None
                    } else {
                        Some(val - 1)
                    }
                } else {
                    None
                };
                console.history_index = new_history_index;

                // Replace input buffer
                match console.history_index {
                    Some(index) => {
                        let input_history: Vec<Line> = console.input_history().cloned().collect();
                        if input_history.len() > 0 {
                            if let Input(input) = &input_history[index] {
                                console.input_buffer = input.clone();
                            }
                        }
                    }
                    None => {
                        console.input_buffer = String::new();
                    }
                }
            }
            Return => {
                let input = {
                    let mut console = gs.ecs.fetch_mut::<Console>();
                    console.history_index = None;

                    let input = console.input_buffer.clone();
                    console.input_buffer.clear();
                    console.history.push(Input(input.clone()));
                    input
                };
                match execute(gs, input.as_str()) {
                    Ok(runstate) => {
                        next_state = runstate;
                    }
                    Err(e) => {
                        let mut console = gs.ecs.fetch_mut::<Console>();
                        console.history.push(Output(e.to_string()));

                        // More details in real console
                        match e {
                            ConsoleError::InvalidArgument {
                                source,
                                expected: _,
                                found: _,
                            } => {
                                console::log(source);
                            }
                            _ => {}
                        }
                    }
                }
            }
            keycode => {
                let mut console = gs.ecs.fetch_mut::<Console>();
                console.history_index = None;

                let c = convert(keycode);
                if c.len() > 0 {
                    console.input_buffer.push_str(c);
                }
            }
        },
    };
    next_state
}

fn execute(gs: &mut State, input: &str) -> Result<RunState, ConsoleError> {
    use Line::*;

    let parts: Vec<&str> = input.split(" ").collect();
    if let Some(&command) = parts.get(0) {
        match command {
            "console" => {
                console_commands(gs, &parts[1..])?;
            }
            "spawn" => {
                spawn_commands(gs, &parts[1..])?;
            }
            "fog" => {
                let mut debug = gs.ecs.fetch_mut::<DebugOptions>();
                debug.fog_off = !debug.fog_off;
            }
            "reveal" => {
                let mut debug = gs.ecs.fetch_mut::<DebugOptions>();
                debug.reveal_hidden = !debug.reveal_hidden;
            }
            "descend" => {
                let mut console = gs.ecs.fetch_mut::<Console>();
                console.history.push(Output("Descending...".into()));
                return Ok(RunState::NextLevel);
            }
            "help" => {
                let mut console = gs.ecs.fetch_mut::<Console>();
                console
                    .history
                    .push(Output("tsk tsk, ain't none of that 'round here...".into()));
            }
            "halp" => {
                let mut console = gs.ecs.fetch_mut::<Console>();
                console
                    .history
                    .push(Output("halp                     - halp".into()));
                console
                    .history
                    .push(Output("fog                      - toggle map fog".into()));
                console.history.push(Output(
                    "reveal                   - toggle reveal hidden".into(),
                ));
                console
                    .history
                    .push(Output("spawn potion             - spawn potion".into()));
                console.history.push(Output(
                    "spawn magicmissile       - spawn magic missile scroll".into(),
                ));
                console.history.push(Output(
                    "spawn fireball           - spawn fireball scroll".into(),
                ));
                console.history.push(Output(
                    "spawn confusion          - spawn confusion scroll".into(),
                ));
                console
                    .history
                    .push(Output("spawn dagger             - spawn dagger".into()));
                console
                    .history
                    .push(Output("spawn shield             - spawn shield".into()));
                console
                    .history
                    .push(Output("spawn longsword          - spawn longsword".into()));
                console.history.push(Output(
                    "spawn towershield        - spawn towershield".into(),
                ));
                console
                    .history
                    .push(Output("spawn trap               - spawn trap".into()));
                console
                    .history
                    .push(Output("descend                  - go down 1 level".into()));
                console.history.push(Output(
                    "console height <lines>   - set console height".into(),
                ));
            }
            c => return Err(ConsoleError::UnknownCommand(c.into())),
        }
    }
    Ok(RunState::Console)
}

fn spawn_commands(gs: &mut State, args: &[&str]) -> Result<(), ConsoleError> {
    let player_pos = {
        let pos = gs.ecs.fetch::<Point>();
        *pos.clone()
    };
    if let Some(&arg) = args.get(0) {
        match arg {
            "potion" => {
                spawner::health_potion(&mut gs.ecs, player_pos.x, player_pos.y);
            }
            "magicmissile" => {
                spawner::magic_missile_scroll(&mut gs.ecs, player_pos.x, player_pos.y);
            }
            "fireball" => {
                spawner::fireball_scroll(&mut gs.ecs, player_pos.x, player_pos.y);
            }
            "confusion" => {
                spawner::confusion_scroll(&mut gs.ecs, player_pos.x, player_pos.y);
            }
            "dagger" => {
                spawner::dagger(&mut gs.ecs, player_pos.x, player_pos.y);
            }
            "shield" => {
                spawner::shield(&mut gs.ecs, player_pos.x, player_pos.y);
            }
            "longsword" => {
                spawner::longsword(&mut gs.ecs, player_pos.x, player_pos.y);
            }
            "towershield" => {
                spawner::tower_shield(&mut gs.ecs, player_pos.x, player_pos.y);
            }
            "trap" => {
                spawner::bear_trap(&mut gs.ecs, player_pos.x, player_pos.y);
            }
            c => return Err(ConsoleError::UnknownCommand(format!("spawn {}", c).into())),
        }
    }
    Ok(())
}

fn console_commands(gs: &mut State, args: &[&str]) -> Result<(), ConsoleError> {
    let mut console = gs.ecs.fetch_mut::<Console>();
    if let Some(&arg) = args.get(0) {
        match arg {
            "height" => {
                let height = args
                    .get(1)
                    .ok_or(ConsoleError::InsufficientArguments(1))
                    .and_then(|h| {
                        h.parse::<usize>()
                            .map_err(|e| ConsoleError::InvalidArgument {
                                source: anyhow::anyhow!(e),
                                expected: "an integer".into(),
                                found: (*h).into(),
                            })
                    })?;
                console.height = max(3, height);
            }
            c => {
                return Err(ConsoleError::UnknownCommand(
                    format!("console {}", c).into(),
                ))
            }
        }
    }
    Ok(())
}

// TODO there must be a better way
fn convert(key: VirtualKeyCode) -> &'static str {
    use VirtualKeyCode::*;
    match key {
        A => "a",
        B => "b",
        C => "c",
        D => "d",
        E => "e",
        F => "f",
        G => "g",
        H => "h",
        I => "i",
        J => "j",
        K => "k",
        L => "l",
        M => "m",
        N => "n",
        O => "o",
        P => "p",
        Q => "q",
        R => "r",
        S => "s",
        T => "t",
        U => "u",
        V => "v",
        W => "w",
        X => "x",
        Y => "y",
        Z => "z",
        Space => " ",
        Key1 => "1",
        Key2 => "2",
        Key3 => "3",
        Key4 => "4",
        Key5 => "5",
        Key6 => "6",
        Key7 => "7",
        Key8 => "8",
        Key9 => "9",
        Key0 => "0",
        Escape
        | F1
        | F2
        | F3
        | F4
        | F5
        | F6
        | F7
        | F8
        | F9
        | F10
        | F11
        | F12
        | F13
        | F14
        | F15
        | F16
        | F17
        | F18
        | F19
        | F20
        | F21
        | F22
        | F23
        | F24
        | Grave
        | Snapshot
        | Scroll
        | Pause
        | Insert
        | Home
        | Delete
        | End
        | PageDown
        | PageUp
        | Left
        | Up
        | Right
        | Down
        | Back
        | Return
        | Compose
        | Caret
        | Numlock
        | Numpad0
        | Numpad1
        | Numpad2
        | Numpad3
        | Numpad4
        | Numpad5
        | Numpad6
        | Numpad7
        | Numpad8
        | Numpad9
        // | NumpadAdd
        // | NumpadDivide
        // | NumpadDecimal
        // | Add
        // | Decimal
        // | Divide
        // | Multiply
        // | Subtract
        | NumpadComma
        | NumpadEnter
        | NumpadEquals
        // | NumpadMultiply
        // | NumpadSubtract
        | AbntC1
        | AbntC2
        | Apostrophe
        | Apps
        // | Asterisk
        | At
        | Ax
        | Backslash
        | Calculator
        | Capital
        | Colon
        | Comma
        | Convert
        | Equals
        | Kana
        | Kanji
        | LAlt
        | LBracket
        | LControl
        | LShift
        | LWin
        | Mail
        | MediaSelect
        | MediaStop
        | Minus
        | Mute
        | MyComputer
        | NavigateForward
        | NavigateBackward
        | NextTrack
        | NoConvert
        | OEM102
        | Period
        | PlayPause
        // | Plus
        | Power
        | PrevTrack
        | RAlt
        | RBracket
        | RControl
        | RShift
        | RWin
        | Semicolon
        | Slash
        | Sleep
        | Stop
        | Sysrq
        | Tab
        | Underline
        | Unlabeled
        | VolumeDown
        | VolumeUp
        | Wake
        | WebBack
        | WebFavorites
        | WebForward
        | WebHome
        | WebRefresh
        | WebSearch
        | WebStop
        | Yen
        | Copy
        | Paste
        | Cut
        | _ => "",
    }
}
