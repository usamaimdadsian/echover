use crate::window::event_loop;

pub fn run() -> Result<(), String> {
    event_loop::run()
}
