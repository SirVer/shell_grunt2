use notify;
use time;

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use task::{Task, RunningTask};

struct Item<'a> {
    last_run_requested: Option<time::PreciseTime>,
    task: &'a Box<Task>,
    running_task: Option<Box<RunningTask>>,
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
                                                                             last_run_requested:
                                                                                 None,
                                                                             task: &task,
                                                                             running_task: None,
                                                                         });
                entry.last_run_requested = Some(time::PreciseTime::now());
                entry.task = &task;
            }
        }

        self.check_for_new_work();
    }

    fn check_for_new_work(&mut self) {
        let mut done = HashSet::new();

        let now = time::PreciseTime::now();
        for (entry_name, entry) in &mut self.work_items {
            if entry.last_run_requested.is_some() &&
               entry.last_run_requested.unwrap().to(now) > entry.task.start_delay() {
                entry.running_task.take().map(|r| r.interrupt());
                entry.running_task = Some(entry.task.run());
                entry.last_run_requested = None;
                continue;
            }

            if entry.running_task.is_some() &&
               entry.running_task
                   .as_mut()
                   .unwrap()
                   .done() {
                done.insert(entry_name.clone());
            }
        }

        for key in &done {
            self.work_items.remove(key);
        }
    }
}
