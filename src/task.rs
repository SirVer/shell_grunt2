use floating_duration::TimeFormat;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path;
use std::process;
use std::thread;
use term;
use time;

pub trait RunningTask {
    fn done(&mut self) -> bool;
    fn wait(self: Box<Self>);
    fn interrupt(self: Box<Self>);
}

pub trait Runnable {
    fn run(&self) -> Box<dyn RunningTask>;
}

pub trait Task: Runnable {
    fn should_run(&self, _: &path::Path) -> bool;
    fn start_delay(&self) -> time::Duration;
}

pub struct ShellCommand {
    pub name: String,
    pub command: String,
    pub work_directory: Option<path::PathBuf>,
}

pub trait ShellTask: Task {
    // Will run the first command, on success the second..
    fn commands(&self) -> Vec<ShellCommand>;
    fn environment(&self) -> Option<HashMap<String, String>>;
    fn redirect_stdout(&self) -> Option<path::PathBuf>;
    fn redirect_stderr(&self) -> Option<path::PathBuf>;
    fn supress_stdout(&self) -> bool;
    fn supress_stderr(&self) -> bool;
}

fn handle_output<R: BufRead, W: Write>(reader: R, echo: bool, mut redirect: Option<BufWriter<W>>) {
    for line in reader.lines() {
        if !line.is_ok() {
            continue;
        }
        let line = line.unwrap();

        // TODO(sirver): My understanding is that \x1b[ should start every ANSI sequence, but I
        // also see \x1b( in my outputs - so I filtered that too. Figure out a more principled way
        // of removing color.
        // for now this is lifted from chalk/ansi-regex.
        lazy_static! {
            static ref REMOVE_ANSI: Regex = Regex::new(
                "[\u{1b}\u{9b}][\\[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-PRZcf-nqry=><]"
            )
            .unwrap();
            static ref REMOVE_SHIFT_INOUT: Regex = Regex::new("[\u{0e}\u{0f}]").unwrap();
        }
        let no_color = REMOVE_ANSI.replace_all(&line, "");
        let no_shift = REMOVE_SHIFT_INOUT.replace_all(&no_color, "");
        redirect
            .as_mut()
            .map(|w| writeln!(w, "{}", no_shift).unwrap());
        if echo {
            println!("{}", line);
        }
    }
}

struct RunningChildState {
    name: String,
    child: process::Child,
    start_time: time::PreciseTime,
    io_threads: Vec<thread::JoinHandle<()>>,
}

struct RunningShellTask {
    commands: Vec<ShellCommand>,
    environment: Option<HashMap<String, String>>,
    echo_stdout: bool,
    redirect_stdout: Option<path::PathBuf>,
    echo_stderr: bool,
    redirect_stderr: Option<path::PathBuf>,
    running_child: Option<RunningChildState>,
    progress_reporter: Box<dyn ProgressReporter>,
}

trait ProgressReporter {
    fn clear_screen(&self);
    fn starting_command(&self, name: &str);
    fn command_finished(&self, name: &str, duration: std::time::Duration, success: bool);
}

/// A pretty printing progress reporter.
struct TermProgressReporter;

impl ProgressReporter for TermProgressReporter {
    fn clear_screen(&self) {
        let mut terminal = term::stdout().unwrap();
        write!(terminal, "\x1b[2J").unwrap(); // Clear the screen.
    }

    fn starting_command(&self, name: &str) {
        let mut terminal = term::stdout().unwrap();
        terminal.fg(term::color::CYAN).unwrap();
        writeln!(terminal, "==> {}", name).unwrap();
        terminal.reset().unwrap();
        terminal.flush().unwrap();
    }

    fn command_finished(&self, name: &str, duration: std::time::Duration, success: bool) {
        let mut terminal = term::stdout().unwrap();
        terminal.fg(term::color::CYAN).unwrap();
        write!(terminal, "==> {}: ", name).unwrap();
        terminal.reset().unwrap();
        if success {
            terminal.fg(term::color::GREEN).unwrap();
            write!(terminal, "Success. ").unwrap();
        } else {
            terminal.fg(term::color::RED).unwrap();
            write!(terminal, "Failed. ").unwrap();
        }
        terminal.reset().unwrap();
        write!(terminal, "({})", TimeFormat(duration)).unwrap();
        writeln!(terminal, "").unwrap();
    }
}

/// A dumb progress reporter for non-interactive shells.
struct DumbProgressReporter;

impl ProgressReporter for DumbProgressReporter {
    fn clear_screen(&self) {}

    fn starting_command(&self, name: &str) {
        println!("==> {}", name);
    }

    fn command_finished(&self, name: &str, duration: std::time::Duration, success: bool) {
        print!("==> {}: ", name);
        if success {
            print!("Success. ");
        } else {
            print!("Failed. ");
        }
        print!("({})", TimeFormat(duration));
        println!("");
    }
}

impl RunningShellTask {
    pub fn spawn(
        commands: Vec<ShellCommand>,
        environment: Option<HashMap<String, String>>,
        echo_stdout: bool,
        redirect_stdout: Option<path::PathBuf>,
        echo_stderr: bool,
        redirect_stderr: Option<path::PathBuf>,
    ) -> Self {
        let progress_reporter: Box<dyn ProgressReporter> = match term::stdout() {
            None => Box::new(DumbProgressReporter {}),
            Some(_) => Box::new(TermProgressReporter {}),
        };

        let mut this = RunningShellTask {
            commands,
            environment,
            echo_stdout,
            redirect_stdout,
            echo_stderr,
            redirect_stderr,
            running_child: None,
            progress_reporter,
        };

        this.progress_reporter.clear_screen();

        this.run_next_command(true);
        this
    }

    fn run_next_command(&mut self, is_first: bool) {
        assert!(self.running_child.is_none());
        if self.commands.is_empty() {
            return;
        }
        let command = self.commands.remove(0);

        // TODO(sirver): This should use something like 'conch-parser', this is quite cheap.
        let args = command.command.split_whitespace().collect::<Vec<&str>>();
        self.progress_reporter.starting_command(&command.name);

        let start_time = time::PreciseTime::now();
        let mut child = {
            let mut child = process::Command::new(args[0]);
            child
                .args(&args[1..])
                .stdin(process::Stdio::inherit())
                .stdout(process::Stdio::piped())
                .stderr(process::Stdio::piped());
            if let Some(path) = command.work_directory {
                child.current_dir(path);
            }
            if let Some(ref environment) = self.environment {
                for (k, v) in environment {
                    child.env(k, v);
                }
            }
            child
                .spawn()
                .unwrap_or_else(|e| panic!("failed to execute: {}", e))
        };

        let mut io_threads = Vec::new();
        let creation_func = |p| {
            OpenOptions::new()
                .write(true)
                .create(is_first)
                .truncate(is_first)
                .append(!is_first)
                .open(p)
        };

        let stdout = BufReader::new(child.stdout.take().unwrap());
        let echo_stdout = self.echo_stdout;
        let redirect_stdout = self
            .redirect_stdout
            .as_ref()
            .map(|path| BufWriter::with_capacity(512, creation_func(path).unwrap()));
        io_threads.push(thread::spawn(move || {
            handle_output(stdout, echo_stdout, redirect_stdout);
        }));
        let stderr = BufReader::new(child.stderr.take().unwrap());
        let echo_stderr = self.echo_stderr;
        let redirect_stderr = self
            .redirect_stderr
            .as_ref()
            .map(|path| BufWriter::with_capacity(512, creation_func(path).unwrap()));
        io_threads.push(thread::spawn(move || {
            handle_output(stderr, echo_stderr, redirect_stderr);
        }));
        self.running_child = Some(RunningChildState {
            name: command.name,
            io_threads,
            child,
            start_time,
        });
    }

    fn current_command_finished(&mut self, success: bool) {
        assert!(self.running_child.is_some());
        let running_child = self.running_child.take().unwrap();

        let duration = running_child
            .start_time
            .to(time::PreciseTime::now())
            .to_std()
            .unwrap();
        self.progress_reporter
            .command_finished(&running_child.name, duration, success);
        if success {
            self.run_next_command(false);
        }
    }
}

impl Drop for RunningShellTask {
    fn drop(&mut self) {
        if let Some(mut running_child) = self.running_child.take() {
            for handle in running_child.io_threads.drain(..) {
                handle.join().unwrap();
            }
        }
    }
}

impl RunningTask for RunningShellTask {
    fn done(&mut self) -> bool {
        if self.running_child.is_none() {
            return true;
        }

        let success = match self
            .running_child
            .as_mut()
            .unwrap()
            .child
            .try_wait()
            .expect("try_wait")
        {
            Some(status) => status.success(),
            None => return false,
        };
        self.current_command_finished(success);
        self.done()
    }

    fn wait(mut self: Box<Self>) {
        if self.done() {
            return;
        }
        self.running_child
            .take()
            .map(|mut r| r.child.wait().expect("wait"));
        self.wait()
    }

    fn interrupt(mut self: Box<Self>) {
        if self.done() {
            return;
        }
        self.running_child
            .take()
            .map(|mut r| r.child.kill().expect("kill"));
    }
}

impl<T: ShellTask> Runnable for T {
    /// Dispatches to 'program' with 'str'.
    fn run(&self) -> Box<dyn RunningTask> {
        Box::new(RunningShellTask::spawn(
            self.commands(),
            self.environment(),
            !self.supress_stdout(),
            self.redirect_stdout(),
            !self.supress_stderr(),
            self.redirect_stderr(),
        ))
    }
}
