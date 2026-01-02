use crate::quad_snd::PlaySoundParams;

pub struct AudioContext;

impl AudioContext {
    pub fn new() -> AudioContext {
        AudioContext
    }
}

pub struct Playback;

impl Playback {
    pub fn stop(self, _ctx: &AudioContext) {}

    pub fn set_volume(&self, _ctx: &AudioContext) {}
}

#[derive(Clone)]
pub struct Sound;

impl Sound {
    pub fn load(_data: &[f32]) -> Sound {
        Sound
    }

    pub fn play(&self, _ctx: &AudioContext, _params: PlaySoundParams) -> Playback {
        Playback
    }

    pub fn stop(&self, _ctx: &AudioContext) {}

    pub fn set_volume(&self, _ctx: &AudioContext, _volume: f32) {}

    pub fn delete(&self, _ctx: &AudioContext) {}
}
