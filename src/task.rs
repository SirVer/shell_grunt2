use floating_duration::TimeFormat;
use regex::Regex;
use std::fs::File;
use std::io::{Write, BufReader, BufRead, BufWriter};
use std::path;
use std::thread;
use std::process;
use term;
use time;

pub trait RunningTask {
    fn done(&mut self) -> bool;
    fn wait(self: Box<Self>);
    fn interrupt(self: Box<Self>);
}

pub trait Runnable {
    fn run(&self) -> Box<RunningTask>;
}

pub trait Task: Runnable {
    fn name(&self) -> String;
    fn should_run(&self, _: &path::Path) -> bool;
    fn start_delay(&self) -> time::Duration;
}

pub trait ShellTask: Task {
    fn command(&self) -> String;
    fn work_directory(&self) -> Option<path::PathBuf>;
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
            static ref REMOVE_ANSI: Regex = Regex::new("[\u{1b}\u{9b}][\\[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-PRZcf-nqry=><]").unwrap();
            static ref REMOVE_SHIFT_INOUT: Regex = Regex::new("[\u{0e}\u{0f}]").unwrap();
        }
        let no_color = REMOVE_ANSI.replace_all(&line, "");
        let no_shift = REMOVE_SHIFT_INOUT.replace_all(&no_color, "");
        redirect.as_mut().map(|w| writeln!(w, "{}", no_shift).unwrap());
        if echo {
            println!("{}", line);
        }
    }
}

struct RunningShellTask {
    child: process::Child,
    start_time: time::PreciseTime,
    done: bool,
    name: String,
    io_threads: Vec<thread::JoinHandle<()>>,
}

impl RunningShellTask {
    pub fn spawn(command: &str,
                 name: String,
                 work_directory: Option<path::PathBuf>,
                 echo_stdout: bool,
                 redirect_stdout: Option<path::PathBuf>,
                 echo_stderr: bool,
                 redirect_stderr: Option<path::PathBuf>)
                 -> Self {
        let args = command.split_whitespace().collect::<Vec<&str>>();
        let mut terminal = term::stdout().unwrap();
        terminal.fg(term::color::CYAN).unwrap();
        write!(terminal, "\x1b[2J").unwrap(); // Clear the screen.
        writeln!(terminal, "==> {}", name).unwrap();
        terminal.reset().unwrap();
        terminal.flush().unwrap();

        let start_time = time::PreciseTime::now();

        let mut child = {
            let mut child = process::Command::new(args[0]);

            child.args(&args[1..])
                .stdin(process::Stdio::inherit())
                .stdout(process::Stdio::piped())
                .stderr(process::Stdio::piped());

            if let Some(path) = work_directory {
                child.current_dir(path);
            }
            child.spawn().unwrap_or_else(|e| panic!("failed to execute: {}", e))
        };

        let mut io_threads = Vec::new();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let redirect_stdout =
            redirect_stdout.map(|path| BufWriter::new(File::create(path).unwrap()));
        io_threads.push(thread::spawn(move || {
            handle_output(stdout, echo_stdout, redirect_stdout);
        }));
        let stderr = BufReader::new(child.stderr.take().unwrap());
        let redirect_stderr =
            redirect_stderr.map(|path| BufWriter::new(File::create(path).unwrap()));
        io_threads.push(thread::spawn(move || {
            handle_output(stderr, echo_stderr, redirect_stderr);
        }));

        RunningShellTask {
            start_time,
            name,
            child,
            done: false,
            io_threads: io_threads,
        }
    }

    fn finish(&mut self, success: bool) {
        self.done = true;

        let duration = self.start_time
            .to(time::PreciseTime::now())
            .to_std()
            .unwrap();
        let mut terminal = term::stdout().unwrap();
        terminal.fg(term::color::CYAN).unwrap();
        write!(terminal, "==> {}: ", self.name).unwrap();
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

impl Drop for RunningShellTask {
    fn drop(&mut self) {
        for handle in self.io_threads.drain(..) {
            handle.join().unwrap();
        }
    }
}

impl RunningTask for RunningShellTask {
    fn done(&mut self) -> bool {
        if self.done {
            return true;
        }

        let success = match self.child.try_wait().expect("try_wait") {
            Some(status) => status.success(),
            None => return false,
        };
        self.finish(success);
        true
    }

    fn wait(mut self: Box<Self>) {
        if self.done {
            return;
        }
        self.child.wait().expect("wait");
    }

    fn interrupt(mut self: Box<Self>) {
        if self.done {
            return;
        }
        self.child.kill().unwrap()
    }
}

impl<T: ShellTask> Runnable for T {
    /// Dispatches to 'program' with 'str'.
    fn run(&self) -> Box<RunningTask> {
        Box::new(RunningShellTask::spawn(&self.command(),
                                         self.name(),
                                         self.work_directory(),
                                         !self.supress_stdout(),
                                         self.redirect_stdout(),
                                         !self.supress_stderr(),
                                         self.redirect_stderr()))
    }
}
