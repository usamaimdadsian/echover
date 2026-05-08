#[allow(dead_code)]
pub trait PlaybackEngine {
    fn load(&mut self, path: &str) -> Result<(), String>;
    fn play(&mut self) -> Result<(), String>;
    fn pause(&mut self) -> Result<(), String>;
    fn toggle(&mut self) -> Result<(), String>;
    fn seek_forward(&mut self, seconds: u64) -> Result<(), String>;
    fn seek_backward(&mut self, seconds: u64) -> Result<(), String>;
    /// Jump directly to an absolute position inside the currently loaded
    /// source. Negative values clamp to zero.
    fn seek_to_ms(&mut self, position_ms: i64) -> Result<(), String>;
    fn is_playing(&self) -> bool;
    fn current_position_ms(&self) -> i64;
}
