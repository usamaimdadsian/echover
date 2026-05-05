use std::{fs::File, time::Duration};

use rodio::source::Source;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

use crate::playback::engine::PlaybackEngine;

pub struct RodioPlaybackEngine {
    _sink: MixerDeviceSink,
    player: Player,
    loaded_path: Option<String>,
    position: Duration,
}

impl RodioPlaybackEngine {
    pub fn new() -> Result<Self, String> {
        let sink = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("failed to open default audio sink: {error}"))?;
        let player = Player::connect_new(sink.mixer());
        player.pause();

        Ok(Self {
            _sink: sink,
            player,
            loaded_path: None,
            position: Duration::from_millis(0),
        })
    }

    fn reload_at_position(&mut self, path: &str, position: Duration) -> Result<(), String> {
        let file =
            File::open(path).map_err(|error| format!("failed to open audio file '{path}': {error}"))?;
        let decoder = Decoder::try_from(file)
            .map_err(|error| format!("failed to decode audio file '{path}': {error}"))?;
        let source = decoder.skip_duration(position);

        self.player.clear();
        self.player.append(source);
        self.player.pause();
        self.position = position;
        self.loaded_path = Some(path.to_owned());
        Ok(())
    }
}

impl PlaybackEngine for RodioPlaybackEngine {
    fn load(&mut self, path: &str) -> Result<(), String> {
        if self.loaded_path.as_deref() == Some(path) {
            return Ok(());
        }
        self.reload_at_position(path, Duration::from_millis(0))
    }

    fn play(&mut self) -> Result<(), String> {
        if self.loaded_path.is_none() {
            return Err("no audio file is loaded".to_owned());
        }

        self.player.play();
        Ok(())
    }

    fn pause(&mut self) -> Result<(), String> {
        self.player.pause();
        Ok(())
    }

    fn toggle(&mut self) -> Result<(), String> {
        if self.loaded_path.is_none() {
            return Err("no audio file is loaded".to_owned());
        }

        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
        Ok(())
    }

    fn seek_forward(&mut self, seconds: u64) -> Result<(), String> {
        let Some(path) = self.loaded_path.clone() else {
            return Err("no audio file is loaded".to_owned());
        };
        let was_playing = !self.player.is_paused();
        let next = self.position.saturating_add(Duration::from_secs(seconds));
        self.reload_at_position(&path, next)?;
        if was_playing {
            self.player.play();
        }
        Ok(())
    }

    fn seek_backward(&mut self, seconds: u64) -> Result<(), String> {
        let Some(path) = self.loaded_path.clone() else {
            return Err("no audio file is loaded".to_owned());
        };
        let was_playing = !self.player.is_paused();
        let rewind = Duration::from_secs(seconds);
        let next = self.position.saturating_sub(rewind);
        self.reload_at_position(&path, next)?;
        if was_playing {
            self.player.play();
        }
        Ok(())
    }

    fn is_playing(&self) -> bool {
        self.loaded_path.is_some() && !self.player.is_paused()
    }

    fn current_position_ms(&self) -> i64 {
        let tracked = self.position.as_millis() as i64;
        tracked.saturating_add(self.player.get_pos().as_millis() as i64)
    }
}
