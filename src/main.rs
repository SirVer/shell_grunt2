extern crate watcher;
extern crate time;

use std::path;
use std::result;

use watcher::Task;

#[derive(Debug)]
pub enum Error {
    SubcommandFailed,
}

pub type Result<T> = result::Result<T, Error>;

// struct Ctags;
// impl Task for Ctags {
    // fn name(&self) -> &str { "Running ctags" }
    // fn command(&self) -> &str { "ctags -R cityblock java" }
    // fn should_run(&self, path: &path::Path) -> bool {
        // match path.extension() {
            // None => return false,
            // Some(ext) => ext,
        // };
        // true
    // }
    // fn start_delay(&self) -> time::Duration {
        // time::Duration::milliseconds(500)
    // }
// }

// struct RebuildCtrPCache {
    // output_file: path::PathBuf,
// }

// impl RebuildCtrPCache {
    // fn new() -> RebuildCtrPCache {
        // RebuildCtrPCache {
            // output_file: path::PathBuf::from(
        // "/usr/local/google/home/hrapp/.cache/ctrlp/%usr%local%google%home%hrapp%prog%sc%src%google3.txt"
            // ),
        // }
    // }
// }

// impl Task for RebuildCtrPCache {
    // fn name(&self) -> &str { "Rebuilding CtrlP cache" }
    // fn should_run(&self, path: &path::Path) -> bool {
        // match path.file_name() {
            // Some(f) => return f.to_string_lossy() != "tags",
            // None => (),
        // }
        // true
    // }
    // fn command(&self) -> &str {
        // "find . -type f \
            // -and -not -iname *.png \
            // -and -not -iname *.jpg \
            // -and -not -iname *.class \
            // -and -not -iname *.jar \
            // -and -not -iregex .*\\.git\\/.* \
            // -and -not -iregex .*\\.gradle\\/.* \
            // -and -not -iregex .*\\java\\/.* \
        // "
    // }

    // fn redirect_stdout(&self) -> Option<&path::Path> {
        // Some(&self.output_file)
    // }

    // fn start_delay(&self) -> time::Duration {
        // time::Duration::milliseconds(500)
    // }
// }

fn main() {
  // let tasks: Vec<Box<Task>> = vec![ Box::new(RebuildCtrPCache::new()), Box::new(Ctags) ];

    let tasks = watcher::lua_task::run_file(path::Path::new("watcher.lua"));
    for t in &tasks {
        println!("#sirver t.name(): {:?}", t.name());
    }
    watcher::loop_forever(tasks);

}
