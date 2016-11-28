extern crate notify;
extern crate time;

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use super::task::Task;

struct Item<'a> {
  last_seen: time::PreciseTime,
  task: &'a Box<Task>,
}

pub struct ShellGrunt2<'a> {
    tasks: &'a[Box<Task>],
    events_rx: mpsc::Receiver<notify::Event>,
    work_items_tx: mpsc::Sender<&'a Box<Task>>,
    work_items_rx: mpsc::Receiver<&'a Box<Task>>,
    work_items: HashMap<String, Item<'a>>
}

impl<'a> ShellGrunt2<'a> {
    pub fn new(tasks: &'a [Box<Task>], events_rx: mpsc::Receiver<notify::Event>) -> ShellGrunt2<'a> {

        let (work_items_tx, work_items_rx) = mpsc::channel::<&'a Box<Task>>();
        ShellGrunt2 {
            tasks: tasks,
            work_items: HashMap::new(),
            events_rx: events_rx,
            work_items_tx: work_items_tx,
            work_items_rx: work_items_rx,
        }
    }

    pub fn spin(&mut self) {
        self.check_for_new_work();

        let ev = match self.events_rx.try_recv() {
            Ok(ev) => ev,
            Err(_) => return,
        };

        if let Some(ref path) = ev.path {
            for task in self.tasks {
                if task.should_run(path) {
                    self.work_items_tx.send(&task).unwrap();
                }
            }
        }
    }

    fn check_for_new_work(&mut self) {
        // See if there is an item available.
        match self.work_items_rx.try_recv() {
            Ok(work_item) => {
                let name = work_item.name();
                let entry =
                    self.work_items.entry(name).or_insert(Item {
                        last_seen: time::PreciseTime::now(),
                        task: work_item,
                    });
                entry.last_seen = time::PreciseTime::now();
                entry.task = work_item;
            },
            Err(err) => {
                match err {
                    mpsc::TryRecvError::Empty => {
                        // Nothing to receive. See if we need to work on some items.
                        let mut done = HashSet::<String>::new();
                        find_tasks_to_run(&mut done, &mut self.work_items);

                        for key in &done {
                            self.work_items.remove(key);
                        }
                    },
                    mpsc::TryRecvError::Disconnected => (),
                }
            }
        };
    }

    pub fn cleanup(self) {
        drop(self.work_items_tx);
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
