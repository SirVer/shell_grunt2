extern crate clap;
extern crate notify;
extern crate shell_grunt2;
extern crate time;
extern crate ctrlc;

use notify::Watcher;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use shell_grunt2::task::{Runnable, RunningTask, Task};
use shell_grunt2::lockfile;
use std::time::Duration;
use std::process;
use std::path;
use std::sync::mpsc;
use std::thread;

struct ReloadWatcherFile {
    file_name: String,
    should_restart_tx: mpsc::Sender<()>,
}

struct RunningReloadWatcherFile;

impl RunningTask for RunningReloadWatcherFile {
    fn done(&mut self) -> bool {
        true
    }
    fn wait(self: Box<Self>) {}
    fn interrupt(self: Box<Self>) {}
}

impl Runnable for ReloadWatcherFile {
    fn run(&self) -> Box<RunningTask> {
        self.should_restart_tx.send(()).unwrap();
        Box::new(RunningReloadWatcherFile {})
    }
}

impl Task for ReloadWatcherFile {
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
    let saw_interrupt_signal = Arc::new(AtomicBool::new(false));
    let r = saw_interrupt_signal.clone();
    ctrlc::set_handler(move || {
        println!("Saw ctrl-c. Terminating!");
        r.store(true, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    'outer: loop {
        println!("Watching file system with tasks from {}", watcher_file);

        // Ideally, the RecommendedWatcher would be owned by ShellGrunt2, but whenever I try that, the
        // tool crashes whenever it should receive an event on the channel. So it needs to stay
        // outside. :(
        let (events_tx, events_rx) = mpsc::channel();
        let mut watcher = notify::watcher(events_tx, Duration::from_millis(50)).unwrap();
        watcher
            .watch(&path::Path::new("."), notify::RecursiveMode::Recursive)
            .unwrap();

        let (should_restart_tx, should_restart_rx) = mpsc::channel();
        let mut tasks: Vec<Box<Task>> = vec![
            Box::new(ReloadWatcherFile {
                file_name: watcher_file.to_string(),
                should_restart_tx,
            }),
        ];
        for task in shell_grunt2::lua_task::run_file(path::Path::new(watcher_file)) {
            tasks.push(task);
        }
        let mut shell_grunt2 = shell_grunt2::ShellGrunt2::new(&tasks, events_rx);

        loop {
            thread::sleep(std::time::Duration::from_millis(50));
            if saw_interrupt_signal.load(Ordering::SeqCst) {
                return;
            }
            match should_restart_rx.try_recv() {
                Err(mpsc::TryRecvError::Empty) => {
                    shell_grunt2.spin();
                }
                Ok(_) | Err(_) => continue 'outer,
            }
        }
    }
}

fn main() {
    let matches = clap::App::new("shell_grunt2")
        .about(
            "Watches the file system and executes commands from a Lua file.",
        )
        .arg(
            clap::Arg::with_name("file")
                .short("f")
                .help("Lua file to use [watcher.lua]"),
        )
        .get_matches();
    let watcher_file = matches.value_of("file").unwrap_or("watcher.lua");

    let _lockfile = match lockfile::Lockfile::new(watcher_file) {
        Ok(lockfile) => lockfile,
        Err(lockfile::AlreadyExists(path)) => {
            println!(
                "Another shell grunt is already running for {}. Delete\n\n    {}\n\nif you sure this is untrue. Exiting.",
                watcher_file,
                path.to_string_lossy()
            );
            process::exit(1);
        }
    };

    watch_file_events(watcher_file);
}
