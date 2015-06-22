extern crate notify;
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

pub struct ShellGrunt2<'a> {
    tasks: &'a[Box<Task>],
    // NOCOM(#sirver): maybe try adding in a &[] view
    worker: thread::JoinGuard<'a, ()>,
    // _watcher: notify::RecommendedWatcher,
    events_rx: mpsc::Receiver<notify::Event>,
    work_items_tx: mpsc::Sender<&'a Box<Task>>,
}

struct Item<'a> {
  last_seen: time::PreciseTime,
  task: &'a Box<Task>,
}


impl<'a> ShellGrunt2<'a> {
    pub fn new(tasks: &'a [Box<Task>], events_rx: mpsc::Receiver<notify::Event>) -> ShellGrunt2<'a> {

        let (work_items_tx, work_items_rx) = mpsc::channel::<&'a Box<Task>>();
        let child: thread::JoinGuard<'a, ()> = thread::scoped(move || {
            let mut work_items = HashMap::new();
            loop {
                // See if there is an item available.
                match work_items_rx.try_recv() {
                    Ok(work_item) => {
                        println!("ALIVE {}:{}", file!(), line!());
                        let name = work_item.name();
                        println!("ALIVE {}:{}", file!(), line!());
                        let entry =
                            work_items.entry(name).or_insert(Item {
                                last_seen: time::PreciseTime::now(),
                                task: work_item,
                            });
                        println!("ALIVE {}:{}", file!(), line!());
                        entry.last_seen = time::PreciseTime::now();
                        println!("ALIVE {}:{}", file!(), line!());
                        entry.task = work_item;
                        println!("ALIVE {}:{}", file!(), line!());
                    },
                    Err(err) => {
                        match err {
                            mpsc::TryRecvError::Empty => {
                                // Nothing to receive. See if we need to work on some items.
                                let mut done = HashSet::<String>::new();
                        println!("ALIVE {}:{}", file!(), line!());
                                find_tasks_to_run(&mut done, &mut work_items);

                                for key in &done {
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

        ShellGrunt2 {
            tasks: tasks,
            worker: child,
            // _watcher: watcher,
            events_rx: events_rx,
            work_items_tx: work_items_tx,
        }
    }

    pub fn spin(&self) {
        let ev = match self.events_rx.recv() {
            Ok(ev) => ev,
            Err(err) => panic!("{}", err),
        };

        if let Some(ref path) = ev.path {
            for task in self.tasks {
                if task.should_run(path) {
                    println!("ALIVE {}:{}", file!(), line!());
                    self.work_items_tx.send(&task).unwrap();
                }
            }
        }
    }
}


fn find_tasks_to_run(done: &mut HashSet<String>,
                             work_items: &mut HashMap<String, Item>) {
    for (entry_name, entry) in work_items {
        if entry.last_seen.to(time::PreciseTime::now()) > entry.task.start_delay() {
            entry.task.run();
            done.insert(entry_name.clone());
        }
    }
}
