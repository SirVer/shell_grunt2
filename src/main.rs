extern crate clap;
extern crate notify;
extern crate shell_grunt2;
extern crate time;

use shell_grunt2::task::{Task, Runnable};
use self::notify::Watcher;
use std::sync::mpsc;
use std::sync::atomic::AtomicBool;
use std::path;
use std::process;

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

struct ReloadWatcherFile {
    file_name: String,
}

impl Runnable for ReloadWatcherFile {
    fn run(&self) {
        process::exit(0);
    }
}

impl Task for ReloadWatcherFile {
    fn name(&self) -> String {
        format!("Reloading {}", self.file_name)
    }

    fn should_run(&self, path: &path::Path) -> bool {
        if let Some(file_name) = path.file_name() {
            return file_name.to_string_lossy() == self.file_name;
        }
        false
    }

    fn start_delay(&self) -> time::Duration {
        time::Duration::milliseconds(0)
    }
}

fn main() {
    let matches = clap::App::new("shell_grunt2")
        .about("Watches the file system and executes commands from a Lua file.")
        .arg(clap::Arg::with_name("file")
            .short("f")
            .help("Lua file to use [watcher.lua]")
        ).get_matches();
    let watcher_file = matches.value_of("file").unwrap_or("watcher.lua");

  // let tasks: Vec<Box<Task>> = vec![ Box::new(RebuildCtrPCache::new()), Box::new(Ctags) ];
    let (events_tx, events_rx) = mpsc::channel();
    let mut watcher: notify::RecommendedWatcher = notify::Watcher::new(events_tx).unwrap();
    watcher.watch(&path::Path::new(".")).unwrap();


    // NOCOM(#sirver): Watch the watch file.
    // NOCOM(#sirver): add command line options.
    let mut tasks: Vec<Box<Task>> = vec![ Box::new(ReloadWatcherFile{
        file_name: watcher_file.to_string()
    }) ];
    for task in shell_grunt2::lua_task::run_file(path::Path::new(watcher_file)) {
        tasks.push(task);
    }
    let shell_grunt2 = shell_grunt2::ShellGrunt2::new(&tasks, events_rx);

    loop {
        shell_grunt2.spin();
    }
}
