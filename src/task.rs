use crossbeam;
use floating_duration::TimeFormat;
use regex::Regex;
use std::fs::File;
use std::io::{Write, BufReader, BufRead, BufWriter};
use std::path;
use std::process;
use term;
use time;

pub trait Runnable {
    fn run(&self);
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

impl<T: ShellTask> Runnable for T {
    /// Dispatches to 'program' with 'str'.
    fn run(&self) {
        let command = self.command();
        let args = command.split_whitespace().collect::<Vec<&str>>();
        let mut terminal = term::stdout().unwrap();
        terminal.fg(term::color::CYAN).unwrap();
        write!(terminal, "\x1b[2J").unwrap(); // Clear the screen.
        writeln!(terminal, "==> {}", self.name()).unwrap();
        terminal.reset().unwrap();
        terminal.flush().unwrap();

        let start_time = time::PreciseTime::now();

        let mut child = {
            let mut child = process::Command::new(args[0]);

            child.args(&args[1..])
                .stdin(process::Stdio::inherit())
                .stdout(process::Stdio::piped())
                .stderr(process::Stdio::piped());

            if let Some(path) = self.work_directory() {
                child.current_dir(path);
            }
            child.spawn()
                .unwrap_or_else(|e| panic!("failed to execute: {}", e))
        };

        {
            let stdout = BufReader::new(child.stdout.as_mut().unwrap());
            let echo_stdout = !self.supress_stdout();
            let redirect_stdout =
                self.redirect_stdout().map(|path| BufWriter::new(File::create(path).unwrap()));
            let stderr = BufReader::new(child.stderr.as_mut().unwrap());
            let echo_stderr = !self.supress_stderr();
            let redirect_stderr =
                self.redirect_stderr().map(|path| BufWriter::new(File::create(path).unwrap()));

            crossbeam::scope(|scope| {
                scope.spawn(move || { handle_output(stdout, echo_stdout, redirect_stdout); });
                scope.spawn(move || { handle_output(stderr, echo_stderr, redirect_stderr); });
            });
        }

        let success = match child.wait().unwrap().code() {
            Some(code) => code == 0,
            None => false,
        };

        let duration = start_time.to(time::PreciseTime::now()).to_std().unwrap();

        terminal.fg(term::color::CYAN).unwrap();
        write!(terminal, "==> {}: ", self.name()).unwrap();
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
