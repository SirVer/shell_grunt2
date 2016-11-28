extern crate clap;
extern crate notify;
extern crate shell_grunt2;
extern crate time;

use self::notify::Watcher;
use shell_grunt2::task::{Task, Runnable};
use std::path;
use std::sync::{mpsc, Mutex};
use std::thread;

// TODO(sirver): This whole thing got way too complicated. The multiple threads are not really
// needed at all, everything can easily be done via polling in a single thread.
struct ReloadWatcherFile {
    file_name: String,
    should_restart_tx: Mutex<mpsc::Sender<()>>,
}

impl Runnable for ReloadWatcherFile {
    fn run(&self) {
        let should_restart_tx = self.should_restart_tx.lock().unwrap(); 
        should_restart_tx.send(()).unwrap();
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

fn watch_file_events(watcher_file: &str) {
    // Ideally, the RecommendedWatcher would be owned by ShellGrunt2, but whenever I try that, the
    // tool crashes whenever it should receive an event on the channel. So it needs to stay
    // outside. :(
    let (events_tx, events_rx) = mpsc::channel();
    let mut watcher: notify::RecommendedWatcher = notify::Watcher::new(events_tx).unwrap();
    watcher.watch(&path::Path::new(".")).unwrap();

    let (should_restart_tx, should_restart_rx) = mpsc::channel();
    let mut tasks: Vec<Box<Task>> = vec![ Box::new(ReloadWatcherFile{
        file_name: watcher_file.to_string(),
        should_restart_tx: Mutex::new(should_restart_tx),
    }) ];
    for task in shell_grunt2::lua_task::run_file(path::Path::new(watcher_file)) {
        tasks.push(task);
    }
    let mut shell_grunt2 = shell_grunt2::ShellGrunt2::new(&tasks, events_rx);
    loop {
        thread::sleep(std::time::Duration::from_millis(50));
        match should_restart_rx.try_recv() {
            Err(mpsc::TryRecvError::Empty) => {
                shell_grunt2.spin();
            },
            Ok (_) | Err(_) => break,
        }
    }
    shell_grunt2.cleanup();
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

    loop {
        println!("Watching file system with tasks from {}", watcher_file);
        watch_file_events(&watcher_file);
    }
}
