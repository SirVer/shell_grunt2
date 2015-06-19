extern crate time;

use std::path;

pub trait Task: Sync {
    fn name(&self) -> &str;
    fn command(&self) -> & str;
    fn should_run(&self, _: &path::Path) -> bool { true }
    fn start_delay(&self) -> time::Duration {
        time::Duration::milliseconds(50)
    }
    fn redirect_stdout(&self) -> Option<&path::Path> { None }
}
