use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use bracket_lib::prelude::console;
use kira::{
    instance::{InstanceSettings, StopInstanceSettings},
    manager::{error::AddSoundError, AudioManager, AudioManagerSettings},
    parameter::tween::Tween,
    sound::{handle::SoundHandle, Sound, SoundSettings},
    CommandError,
};
use ringbuf::{Consumer, Producer, RingBuffer};
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SoundError {
    #[error("Sound not loaded: {0}")]
    NotLoaded(String),
    #[error("Sound handle error: {0}")]
    CommandError(#[from] CommandError),
}

pub struct SoundResource {
    audio_manager: Arc<Mutex<AudioManager>>,

    sounds: HashMap<String, SoundHandle>,
    tx: Arc<Mutex<Producer<(Asset, Sound)>>>,
    rx: Consumer<(Asset, Sound)>,

    loading: HashSet<Asset>,
    playing: HashSet<Asset>,

    // Handle music differently, TODO overhaul effects in a similar manner with enum
    music_handles: HashMap<Music, SoundHandle>,
    current_music: Option<Music>,
}

impl Default for SoundResource {
    fn default() -> Self {
        let audio_manager = AudioManager::new(AudioManagerSettings::default())
            .expect("Unable to create audio manager");
        let (tx, rx) = RingBuffer::<(Asset, Sound)>::new(20).split();
        Self {
            audio_manager: Arc::new(Mutex::new(audio_manager)),
            sounds: HashMap::new(),
            loading: HashSet::new(),
            playing: HashSet::new(),
            tx: Arc::new(Mutex::new(tx)),
            rx,
            music_handles: HashMap::new(),
            current_music: None,
        }
    }
}

impl SoundResource {
    pub fn finished_loading(&self) -> bool {
        self.loading.is_empty()
    }

    pub fn handle_load_queue(&mut self) -> Result<(), AddSoundError> {
        match self.audio_manager.lock() {
            Ok(mut audio_manager) => {
                while let Some(loaded) = self.rx.pop() {
                    self.loading.remove(&loaded.0);
                    match loaded.0 {
                        Asset::Effect(s) => {
                            let sound_handle = audio_manager.add_sound(loaded.1)?;
                            self.sounds.insert(s, sound_handle);
                        }
                        Asset::Music(m) => {
                            let sound_handle = audio_manager.add_sound(loaded.1)?;
                            self.music_handles.insert(m, sound_handle);
                        }
                    };
                }
            }
            Err(e) => console::log(format!("Mutex poisoned: {}", e)),
        }
        Ok(())
    }

    //TODO rename to effect
    pub fn load_audio(&mut self, url: &'static str) {
        let asset = Asset::Effect(url.into());
        if self.loading.get(&asset).is_some() {
            // We're already trying to load it
            return;
        } else if self.sounds.get(url).is_some() {
            // Was loaded previously, ignore
            console::log("Sound already loaded");
            return;
        }
        console::log(format!("Loading sound: {}", url));

        self.loading.insert(asset.clone());

        let parc = self.tx.clone();
        load_audio_data(url, SoundSettings::default(), move |s| {
            let mut pushed = false;
            while !pushed {
                match parc.lock() {
                    Ok(mut producer) => {
                        match producer.push((asset.clone(), s.clone())) {
                            Ok(_) => pushed = true,
                            Err(_) => {
                                // Continue trying
                            }
                        }
                    }
                    Err(e) => console::log(format!("Mutex poisoned: {}", e)),
                }
            }
        });
    }

    pub fn play_sound(&mut self, url: &str, settings: InstanceSettings) -> Result<(), SoundError> {
        let asset = Asset::Effect(url.into());
        if let Some(sound) = self.sounds.get_mut(url) {
            self.playing.insert(asset);
            // TODO handle error
            sound.play(settings).unwrap();
            Ok(())
        } else {
            Err(SoundError::NotLoaded(url.into()))
        }
    }

    pub fn stop_sound(
        &mut self,
        url: &str,
        settings: StopInstanceSettings,
    ) -> Result<(), SoundError> {
        let asset = Asset::Effect(url.into());
        if let Some(sound) = self.sounds.get_mut(url) {
            // TODO handle error
            sound.stop(settings).unwrap();
            self.playing.remove(&asset);
            Ok(())
        } else {
            Err(SoundError::NotLoaded(url.into()))
        }
    }

    pub fn load_music(&mut self) {
        for music in Music::iter() {
            let file = music.filename();
            let asset = Asset::Music(music);
            if self.loading.get(&asset).is_some() {
                // We're already trying to load it
                return;
            } else if self.music_handles.get(&music).is_some() {
                // Was loaded previously, ignore
                console::log(format!("{} already loaded", file));
                return;
            }
            console::log(format!("Loading asset: {}", file));

            self.loading.insert(asset.clone());

            let parc = self.tx.clone();
            load_audio_data(file, SoundSettings::default(), move |s| {
                let mut pushed = false;
                while !pushed {
                    match parc.lock() {
                        Ok(mut producer) => {
                            match producer.push((asset.clone(), s.clone())) {
                                Ok(_) => pushed = true,
                                Err(_) => {
                                    // Continue trying
                                }
                            }
                        }
                        Err(e) => console::log(format!("Mutex poisoned: {}", e)),
                    }
                }
            });
        }
    }

    pub fn switch_music(&mut self, music: Music) -> Result<(), SoundError> {
        let tween = Tween::linear(5.0);
        if let Some(current) = self.current_music {
            // We're already playing that, dummy..
            if current == music {
                return Ok(());
            }
            if let Some(current_handle) = self.music_handles.get_mut(&current) {
                self.current_music = None;
                current_handle.stop(StopInstanceSettings::new().fade_tween(tween))?;
            }
        }

        if let Some(next) = self.music_handles.get_mut(&music) {
            self.current_music = Some(music);
            next.play(
                InstanceSettings::default()
                    .loop_start(0f64)
                    .fade_in_tween(tween),
            )?;
        }

        Ok(())
    }

    pub fn stop_music(&mut self, settings: StopInstanceSettings) -> Result<(), SoundError> {
        if let Some(current) = self.current_music {
            if let Some(music_handle) = self.music_handles.get_mut(&current) {
                // TODO handle error
                music_handle.stop(settings)?;
                self.playing.remove(&Asset::Music(current));
                Ok(())
            } else {
                Err(SoundError::NotLoaded(current.filename().into()))
            }
        } else {
            console::log("Attempted stop while no music is playing");
            Ok(())
        }
    }

    pub fn stop_all_sounds(&mut self) {
        for asset in self.playing.drain() {
            match asset {
                Asset::Effect(s) => {
                    let handle = self.sounds.get_mut(&s).unwrap();
                    handle.stop(StopInstanceSettings::default()).unwrap();
                }
                Asset::Music(m) => {
                    // handle music differently
                    // let handle = self.music_handles.get_mut(&m).unwrap();
                    // handle.stop(StopInstanceSettings::default()).unwrap();
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Asset {
    Effect(String),
    Music(Music),
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Hash, Debug, EnumIter)]
pub enum Music {
    Abyss,
    GameOver,
}

impl Music {
    pub fn filename(&self) -> &'static str {
        match self {
            Music::Abyss => "assets/audio/roguelike_abyss.ogg",
            Music::GameOver => "assets/audio/roguelike_abyss.ogg", // TODO make replacement
        }
    }
}

pub struct DesireMusic {
    pub music: Option<Music>,
    pub stop: bool,
}

#[cfg(not(target_arch = "wasm32"))]
fn load_audio_data(
    uri: &'static str,
    settings: SoundSettings,
    callback: impl FnOnce(Sound) + 'static,
) {
    let sound = Sound::from_file(uri, settings)
        .expect(format!("Unable to read sound file {}", uri).as_str());
    callback(sound)
}

#[cfg(target_arch = "wasm32")]
use js_sys::ArrayBuffer;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::{AudioBuffer, AudioContext, Request, RequestInit, RequestMode, Response};

#[cfg(target_arch = "wasm32")]
fn load_audio_data(
    url: &'static str,
    settings: SoundSettings,
    callback: impl FnOnce(Sound) + 'static,
) {
    std::mem::drop(wasm_bindgen_futures::future_to_promise(
        load_audio_data_async(url, settings, callback),
    ));
}

#[cfg(target_arch = "wasm32")]
async fn load_audio_data_async(
    url: &'static str,
    settings: SoundSettings,
    callback: impl FnOnce(Sound) + 'static,
) -> Result<JsValue, JsValue> {
    let audio_ctx = AudioContext::new()?;

    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(&url, &opts)?;

    request.headers().set("Accept", "audio/ogg")?;

    let window = web_sys::window().ok_or_else(|| JsValue::from("could not get window handle"))?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

    let resp: Response = resp_value.dyn_into()?;

    let encoded = JsFuture::from(resp.array_buffer()?).await?;
    let encoded: ArrayBuffer = encoded.dyn_into()?;

    let decoded = JsFuture::from(audio_ctx.decode_audio_data(&encoded)?).await?;
    let decoded: AudioBuffer = decoded.dyn_into()?;

    let left = decoded.get_channel_data(0)?;
    let right = decoded.get_channel_data(1)?;

    let frames = left
        .iter()
        .zip(right.iter())
        .map(|(&left, &right)| kira::Frame { left, right })
        .collect();

    callback(Sound::from_frames(
        decoded.sample_rate() as u32,
        frames,
        settings,
    ));

    Ok(JsValue::undefined())
}
