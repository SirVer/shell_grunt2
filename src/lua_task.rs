extern crate libc;
extern crate lua;
extern crate time;

use self::lua::ffi::lua_State;
use std::path;
use std::rc::Rc;
use std::cell::RefCell;
use task::{Task, ShellTask, ShellCommand};

// TODO(sirver): This whole file is quite the hack. If a LuaDictionary would get a proper
// abstraction, this could be expressed more tightly. This is a bit tricky with the correct
// lifetimes.

fn get_value_in_dict(key: &str, state: &mut lua::State) {
    // S: D
    state.push_string(key); // S: D <key>
    state.get_table(-2); // S: D <value>
}

fn pop_string(state: &mut lua::State) -> Option<String> {
    let rv = if state.is_string(-1) {
        Some(state.check_string(-1).to_string())
    } else {
        None
    };
    state.pop(1);
    rv
}

fn pop_int(state: &mut lua::State) -> Option<i64> {
    let rv = if state.is_integer(-1) {
        Some(state.check_integer(-1))
    } else {
        None
    };
    state.pop(1);
    rv
}

fn pop_bool(state: &mut lua::State) -> Option<bool> {
    let rv = if state.is_bool(-1) {
        Some(state.to_bool(-1))
    } else {
        None
    };
    state.pop(1);
    rv
}

struct LuaTask {
    state: Rc<RefCell<lua::State>>,
    key: i64,
}

impl LuaTask {
    fn new(state: Rc<RefCell<lua::State>>, key: i64) -> LuaTask {
        LuaTask {
            state: state,
            key: key,
        }
    }

    fn get_value_in_our_dict(&self, key: &str, state: &mut lua::State) {
        // S: D
        state.push_integer(self.key); // S: D key
        state.get_table(1); // S: D d
        state.push_string(key); // S: D d <key>
        state.get_table(-2); // S: D d <value>
    }

    fn get_string(&self, key: &str) -> Option<String> {
        let mut state = self.state.borrow_mut();
        self.get_value_in_our_dict(key, &mut state); // S: D d <value>
        let rv = pop_string(&mut state);
        state.pop(1);
        rv
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        let mut state = self.state.borrow_mut();
        self.get_value_in_our_dict(key, &mut state); // S: D d <value>
        let rv = pop_int(&mut state);
        state.pop(1);
        rv
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        let mut state = self.state.borrow_mut();
        self.get_value_in_our_dict(key, &mut state); // S: D d <value>
        let rv = pop_bool(&mut state);
        state.pop(1);
        rv
    }
}

impl Task for LuaTask {
    fn should_run(&self, path: &path::Path) -> bool {
        let mut state = self.state.borrow_mut();
        self.get_value_in_our_dict("should_run", &mut state);
        if state.is_nil(-1) {
            // should_run not in dictionary.
            state.pop(2);
            return true;
        }
        state.push_string(&path.to_string_lossy()); // S: D d function path
        state.call(1, 1); // S: D d <result>
        let rv = state.to_bool(-1);
        state.pop(2);
        rv
    }

    fn start_delay(&self) -> time::Duration {
        self.get_int("start_delay")
            .map(time::Duration::milliseconds)
            .unwrap_or(time::Duration::milliseconds(50))
    }
}

impl ShellTask for LuaTask {
    // TODO(sirver): If there is only one command in a chain, a short form should be acceptable in
    // the Lua file.
    fn commands(&self) -> Vec<ShellCommand> {
        let mut state = self.state.borrow_mut();
        self.get_value_in_our_dict("commands", &mut state);
        if state.is_nil(-1) {
            panic!("Expected commands, but was not found.");
        }
        let mut result = Vec::new();
        state.push_nil(); // S: D "commands dict" nil
        while state.next(-2) {
            // S: D "commands dict" key value
            get_value_in_dict("name", &mut state);
            let name = pop_string(&mut state).expect("name in commands.");
            get_value_in_dict("command", &mut state);
            let command = pop_string(&mut state).expect("command in commands.");
            get_value_in_dict("work_directory", &mut state);
            let work_directory = pop_string(&mut state).map(path::PathBuf::from);
            state.pop(1); // S: D "commands dict" key
            result.push(ShellCommand {
                            name,
                            command,
                            work_directory,
                        });
        }
        state.pop(1); // S: D
        result
    }

    fn redirect_stdout(&self) -> Option<path::PathBuf> {
        self.get_string("redirect_stdout").map(path::PathBuf::from)
    }


    fn redirect_stderr(&self) -> Option<path::PathBuf> {
        self.get_string("redirect_stderr").map(path::PathBuf::from)
    }

    fn supress_stderr(&self) -> bool {
        self.get_bool("suppress_stderr").unwrap_or(false)
    }

    fn supress_stdout(&self) -> bool {
        self.get_bool("suppress_stdout").unwrap_or(false)
    }
}

#[allow(non_snake_case)]
unsafe extern "C" fn basename(L: *mut lua_State) -> libc::c_int {
    let mut state = lua::State::from_ptr(L);
    let string: String = state.check_string(-1).to_string();
    let p = path::Path::new(&string);
    match p.file_name() {
        Some(s) => {
            state.push_string(&s.to_string_lossy());
            1
        }
        None => 0,
    }
}

#[allow(non_snake_case)]
unsafe extern "C" fn dirname(L: *mut lua_State) -> libc::c_int {
    let mut state = lua::State::from_ptr(L);
    let string: String = state.check_string(-1).to_string();
    let p = path::Path::new(&string);
    match p.parent() {
        Some(s) => {
            state.push_string(&s.to_string_lossy());
            1
        }
        None => 0,
    }
}

#[allow(non_snake_case)]
unsafe extern "C" fn ext(L: *mut lua_State) -> libc::c_int {
    let mut state = lua::State::from_ptr(L);
    let string: String = state.check_string(-1).to_string();
    let p = path::Path::new(&string);
    match p.extension() {
        Some(ext) => {
            state.push_string(&ext.to_string_lossy());
            1
        }
        None => 0,
    }
}

fn inject_path_functions(state: &mut lua::State) {
    state.get_global("string"); // S: <string>

    state.push_string("ext");
    state.push_fn(Some(ext));
    state.set_table(-3);

    state.push_string("basename");
    state.push_fn(Some(basename));
    state.set_table(-3);

    state.push_string("dirname");
    state.push_fn(Some(dirname));
    state.set_table(-3);

    state.pop(1);
}

pub fn run_file(path: &path::Path) -> Vec<Box<Task>> {
    let mut state = lua::State::new();
    state.open_libs();

    let path_utf8 = path.to_string_lossy();
    match state.do_file(&path_utf8) {
        lua::ThreadStatus::Ok => (),
        _ => {
            let error_message = pop_string(&mut state).unwrap();
            panic!("{}", error_message);
        }
    };

    inject_path_functions(&mut state);

    let mut tasks = Vec::new();
    let state_rc = Rc::new(RefCell::new(state));
    let mut state = state_rc.borrow_mut();

    state.push_nil(); // S: D nil
    while state.next(-2) {
        let key = state.check_integer(-2); // S: D key value
        state.pop(1); // S: D key
        tasks.push(Box::new(LuaTask::new(state_rc.clone(), key)) as Box<Task>);
    }
    // S: D
    tasks
}
