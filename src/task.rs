extern crate time;

use std::path;

pub trait Task: Sync {
    fn name(&self) -> String;
    fn command(&self) -> String;
    fn should_run(&self, _: &path::Path) -> bool;
    fn redirect_stdout(&self) -> Option<path::PathBuf>;
    fn redirect_stderr(&self) -> Option<path::PathBuf>;
    fn start_delay(&self) -> time::Duration;
}
