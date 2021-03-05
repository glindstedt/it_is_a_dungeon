use bracket_lib::prelude::console;
use flume::{Receiver, Sender};
use kira::{
    instance::{InstanceSettings, StopInstanceSettings},
    manager::{error::AddSoundError, AudioManager},
    sound::{handle::SoundHandle, Sound, SoundSettings},
};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SoundError {
    #[error("Sound not loaded: {0}")]
    NotLoaded(String),
}

pub struct SoundResource {
    sounds: HashMap<String, SoundHandle>,
    tx: Sender<(String, Sound)>,
    rx: Receiver<(String, Sound)>,
    loading: HashSet<String>,
    playing: HashSet<String>,
}

impl Default for SoundResource {
    fn default() -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            sounds: HashMap::new(),
            loading: HashSet::new(),
            playing: HashSet::new(),
            tx,
            rx,
        }
    }
}

impl SoundResource {
    pub fn finished_loading(&self) -> bool {
        self.loading.is_empty()
    }

    pub fn handle_load_queue(
        &mut self,
        audio_manager: &mut AudioManager,
    ) -> Result<(), AddSoundError> {
        while let Ok(loaded) = self.rx.try_recv() {
            self.loading.remove(&loaded.0);
            let sound_handle = audio_manager.add_sound(loaded.1)?;
            self.sounds.insert(loaded.0, sound_handle);
        }
        Ok(())
    }

    pub fn load_audio(&mut self, url: &'static str) {
        if self.loading.get(url).is_some() {
            // We're already trying to load it
            return;
        } else if self.sounds.get(url).is_some() {
            // Was loaded previously, ignore
            console::log("Sound already loaded");
            return;
        }
        console::log(format!("Loading sound: {}", url));

        let sound_queue = self.tx.clone();
        self.loading.insert(url.into());
        load_audio_data(url, SoundSettings::default(), move |s| {
            match sound_queue.send((url.into(), s)) {
                Ok(_) => {
                    console::log("Added sound");
                }
                Err(e) => {
                    console::log(format!("Failed to add sound: {}", e));
                }
            }
        });
    }

    pub fn play_sound(&mut self, url: &str, settings: InstanceSettings) -> Result<(), SoundError> {
        if let Some(sound) = self.sounds.get_mut(url) {
            self.playing.insert(url.into());
            // TODO handle error
            sound.play(settings).unwrap();
            Ok(())
        } else {
            Err(SoundError::NotLoaded(url.into()))
        }
    }

    pub fn stop_all_sounds(&mut self) {
        for sound_url in self.playing.drain() {
            let handle = self.sounds.get_mut(&sound_url).unwrap();
            handle.stop(StopInstanceSettings::default()).unwrap();
        }
    }
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
