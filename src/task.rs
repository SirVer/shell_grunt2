extern crate term;
extern crate time;

use std::fs::File;
use std::io::{Write, Read};
use std::path;
use std::process;

pub trait Runnable {
    fn run(&self);
}

pub trait Task: Runnable {
    // NOCOM(#sirver): Return 'a &str?
    fn name(&self) -> String;
    fn should_run(&self, _: &path::Path) -> bool;
    fn start_delay(&self) -> time::Duration;
}

pub trait ShellTask: Task {
    fn command(&self) -> String;
    fn redirect_stdout(&self) -> Option<path::PathBuf>;
    fn redirect_stderr(&self) -> Option<path::PathBuf>;
}

impl<T: ShellTask> Runnable for T {
    /// Dispatches to 'program' with 'str'.
    fn run(&self) {
        let command = self.command();
        let args = command.split_whitespace().collect::<Vec<&str>>();
        let mut terminal = term::stdout().unwrap();
        write!(terminal, "{} ... ", self.name()).unwrap();
        terminal.flush().unwrap();

        let redirect_stdout = self.redirect_stdout();
        let redirect_stderr = self.redirect_stderr();
        let mut child = process::Command::new(args[0])
            .args(&args[1..])
            .stdin(process::Stdio::inherit())
            .stdout(if redirect_stdout.is_some() {
                process::Stdio::piped()
            } else {
                process::Stdio::null()
            })
            .stderr(if redirect_stderr.is_some() {
                process::Stdio::piped()
            } else {
                process::Stdio::null()
            })
            .spawn()
            .unwrap_or_else(|e| {
                panic!("failed to execute: {}", e)
            });

        let success =
        match child.wait().unwrap().code() {
            Some(code) => {
                code == 0
            },
            None => false,
        };

        if success {
            terminal.fg(term::color::GREEN).unwrap();
            write!(terminal, "Success. ").unwrap();
        } else {
            terminal.fg(term::color::RED).unwrap();
            write!(terminal, "Failed. ").unwrap();
        }
        terminal.reset().unwrap();
        writeln!(terminal, "").unwrap();

        if let Some(redirect_stdout) = redirect_stdout {
            let mut s = String::new();
            child.stdout.unwrap().read_to_string(&mut s).unwrap();
            let mut file = File::create(redirect_stdout).unwrap();
            file.write_all(&s.into_bytes()).unwrap();
        }

        if let Some(redirect_stderr) = redirect_stderr {
            let mut s = String::new();
            child.stderr.unwrap().read_to_string(&mut s).unwrap();
            let mut file = File::create(redirect_stderr).unwrap();
            file.write_all(&s.into_bytes()).unwrap();
        }
    }
}
