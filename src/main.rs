#![feature(scoped)]

extern crate notify;
extern crate term;
extern crate time;

use notify::{RecommendedWatcher, Watcher};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Write, Read};
use std::path;
use std::process;
use std::result;
use std::sync::mpsc;
use std::thread;

#[derive(Debug)]
pub enum Error {
    SubcommandFailed,
}

pub type Result<T> = result::Result<T, Error>;

trait Task: Sync {
    fn name(&self) -> &str;
    fn command(&self) -> & str;
    fn should_run(&self, _: &path::Path) -> bool { true }
    fn start_delay(&self) -> time::Duration {
        time::Duration::milliseconds(50)
    }
    fn redirect_stdout(&self) -> Option<&path::Path> { None }
}

struct Ctags;
impl Task for Ctags {
    fn name(&self) -> &str { "Running ctags" }
    fn command(&self) -> &str { "ctags -R cityblock java" }
    fn should_run(&self, path: &path::Path) -> bool {
        match path.extension() {
            None => return false,
            Some(ext) => ext,
        };
        true
    }
    fn start_delay(&self) -> time::Duration {
        time::Duration::milliseconds(500)
    }
}

struct RebuildCtrPCache {
    output_file: path::PathBuf,
}

impl RebuildCtrPCache {
    fn new() -> RebuildCtrPCache {
        RebuildCtrPCache {
            output_file: path::PathBuf::from(
        "/usr/local/google/home/hrapp/.cache/ctrlp/%usr%local%google%home%hrapp%prog%sc%src%google3.txt"
            ),
        }
    }
}

impl Task for RebuildCtrPCache {
    fn name(&self) -> &str { "Rebuilding CtrlP cache" }
    fn command(&self) -> &str {
        "find . -type f \
            -and -not -iname *.png \
            -and -not -iname *.jpg \
            -and -not -iname *.class \
            -and -not -iname *.jar \
            -and -not -iregex .*\\.git\\/.* \
            -and -not -iregex .*\\.gradle\\/.* \
            -and -not -iregex .*\\java\\/.* \
        "
    }

    fn redirect_stdout(&self) -> Option<&path::Path> {
        Some(&self.output_file)
    }

    fn start_delay(&self) -> time::Duration {
        time::Duration::milliseconds(500)
    }
}

/// Dispatches to 'program' with 'str'. 'print' decides if the command lines are echoed.
fn run_task(task: &Task) {
    let args = task.command().split_whitespace().collect::<Vec<&str>>();
    let mut terminal = term::stdout().unwrap();
    write!(terminal, "{} ... ", task.name()).unwrap();
    terminal.flush().unwrap();

    let redirect_stdout = task.redirect_stdout();
    let mut child = process::Command::new(args[0])
        .args(&args[1..])
        .stdin(process::Stdio::inherit())
        .stdout(if redirect_stdout.is_some() {
            process::Stdio::piped()
        } else {
            process::Stdio::null()
        })
        .stderr(process::Stdio::null())
        .spawn()
        .unwrap_or_else(|e| {
            panic!("failed to execute: {}", e)
        });

    match child.wait().unwrap().code() {
        Some(0) => {
            terminal.fg(term::color::GREEN).unwrap();
            write!(terminal, "Success. ").unwrap();

            if let Some(redirect_stdout) = redirect_stdout {
                let mut s = String::new();
                child.stdout.unwrap().read_to_string(&mut s).unwrap();
                let mut file = File::create(redirect_stdout).unwrap();
                file.write_all(&s.into_bytes()).unwrap();
            }
        }
        _ => {
            terminal.fg(term::color::RED).unwrap();
            write!(terminal, "Failed. ").unwrap();
        }
    }
    terminal.reset().unwrap();
    writeln!(terminal, "").unwrap();

}

struct Item<'a> {
  last_seen: time::PreciseTime,
  task: &'a Box<Task>,
}

fn find_tasks_to_run<'a, 'b>(done: &'a mut HashSet<&'b str>,
                             work_items: &'a mut HashMap<&'b str, Item>) {
    for (entry_name, entry) in work_items {
        if entry.last_seen.to(time::PreciseTime::now()) > entry.task.start_delay() {
            run_task(&**entry.task);
            done.insert(entry_name);
        }
    }
}

fn main() {
  let (events_tx, events_rx) = mpsc::channel();
  let mut watcher: RecommendedWatcher = Watcher::new(events_tx).unwrap();

  let tasks: Vec<Box<Task>> = vec![ Box::new(Ctags), Box::new(RebuildCtrPCache::new()) ];

  watcher.watch(&path::Path::new(".")).unwrap();

  let (work_items_tx, work_items_rx) = mpsc::channel::<&Box<Task>>();
  let _guard = thread::scoped(move || {
      let mut work_items = HashMap::new();
      loop {
          // See if there is an item available.
          match work_items_rx.try_recv() {
              Ok(work_item) => {
                  let entry =
                      work_items.entry(work_item.name()).or_insert(Item {
                          last_seen: time::PreciseTime::now(),
                          task: work_item,
                      });
                  entry.last_seen = time::PreciseTime::now();
                  entry.task = work_item;
                  println!("#hrapp work_item.name(): {:?}", work_item.name());
              }
              Err(err) => {
                  match err {
                      mpsc::TryRecvError::Empty => {
                          // Nothing to receive. See if we need to work on some items.
                          let mut done = HashSet::new();
                          find_tasks_to_run(&mut done, &mut work_items);

                          for key in done {
                              work_items.remove(key);
                          }
                          thread::sleep_ms(250);
                      },
                      mpsc::TryRecvError::Disconnected => break,
                  }
              }
          };
      }
  });

  loop {
      // You'll probably want to do that in a loop. The type to match for is
      // notify::Event, look at src/lib.rs for details.
      let ev = match events_rx.recv() {
          Ok(ev) => ev,
          Err(err) => panic!("{}", err),
      };

      if let Some(ref path) = ev.path {
          for task in &tasks {
              if task.should_run(path) {
                  work_items_tx.send(task).unwrap();
              }
          }
      }
  }
}
