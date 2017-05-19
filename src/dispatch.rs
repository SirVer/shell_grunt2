use notify;
use time;

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use super::task::Task;

struct Item<'a> {
    last_seen: time::PreciseTime,
    task: &'a Box<Task>,
}

pub struct ShellGrunt2<'a> {
    tasks: &'a [Box<Task>],
    events_rx: mpsc::Receiver<notify::DebouncedEvent>,
    work_items: HashMap<String, Item<'a>>,
}

impl<'a> ShellGrunt2<'a> {
    pub fn new(tasks: &'a [Box<Task>],
               events_rx: mpsc::Receiver<notify::DebouncedEvent>)
               -> ShellGrunt2<'a> {

        ShellGrunt2 {
            tasks: tasks,
            work_items: HashMap::new(),
            events_rx: events_rx,
        }
    }

    pub fn spin(&mut self) {
        while let Ok(ev) = self.events_rx.try_recv() {
            use notify::DebouncedEvent::*;

            let path = match ev {
                NoticeWrite(path) |
                    NoticeRemove(path) |
                    Create(path) |
                    Write(path) |
                    Remove(path) => path,

                    Rename(_, new_path) => new_path,

                    Error(err, path) => {
                        println!("Ignored error: {:?}, ({:?})", err, path);
                        continue;
                    }
                Rescan | Chmod(_) => continue,
            };
            for task in self.tasks {
                if !task.should_run(&path) {
                    continue;
                }

                let entry = self.work_items.entry(task.name()).or_insert(Item {
                    last_seen:
                        time::PreciseTime::now(),
                        task: &task,
                });
                entry.last_seen = time::PreciseTime::now();
                entry.task = &task;
            }
        }

        self.check_for_new_work();
    }

    fn check_for_new_work(&mut self) {
        let mut done = HashSet::new();

        let now = time::PreciseTime::now();
        for (entry_name, entry) in &self.work_items {
            if entry.last_seen.to(now) > entry.task.start_delay() {
                entry.task.run();
                done.insert(entry_name.to_string());
            }
        }

        for key in &done {
            self.work_items.remove(key);
        }
    }
}
