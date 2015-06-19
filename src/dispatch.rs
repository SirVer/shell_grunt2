extern crate notify;
extern crate term;
extern crate time;

use self::notify::Watcher;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Write, Read};
use std::path;
use std::process;
use std::sync::mpsc;
use std::thread;
use super::task::Task;

pub fn loop_forever(tasks: Vec<Box<Task>>) {
  let (events_tx, events_rx) = mpsc::channel();
  let mut watcher: notify::RecommendedWatcher = notify::Watcher::new(events_tx).unwrap();

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

/// Dispatches to 'program' with 'str'.
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
