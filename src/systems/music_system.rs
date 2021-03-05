use bracket_lib::prelude::console;
use kira::{instance::StopInstanceSettings, parameter::tween::Tween};
use specs::prelude::*;

use crate::audio::{DesireMusic, SoundResource};

pub struct MusicSystem {}

impl<'a> System<'a> for MusicSystem {
    type SystemData = (WriteExpect<'a, DesireMusic>, WriteExpect<'a, SoundResource>);

    fn run(&mut self, data: Self::SystemData) {
        let (mut desire, mut sounds) = data;

        if desire.stop {
            match sounds.stop_music(StopInstanceSettings::new().fade_tween(Tween::linear(4.0))) {
                Ok(_) => {}
                Err(e) => {
                    console::log(format!("Failed to stop music: {}", e));
                }
            };
        } else {
            if let Some(music) = desire.music {
                match sounds.switch_music(music) {
                    Ok(_) => {}
                    Err(e) => {
                        console::log(format!("Failed to switch music: {}", e));
                    }
                }
            }
        }

        // Always reset
        desire.music = None;
        desire.stop = false;
    }
}
