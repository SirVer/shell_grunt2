extern crate libc;
extern crate lua;
extern crate time;

use self::lua::ffi::lua_State;
use std::path;
use std::rc::Rc;
use std::cell::RefCell;
use task::{Task, ShellTask};


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

    fn push_value(&self, key: &str, state: &mut lua::State) {
        // S: D
        state.push_integer(self.key); // S: D key
        state.get_table(1); // S: D d
        state.push_string(key); // S: D d <key>
        state.get_table(-2); // S: D d <value>
    }

    fn get_string(&self, key: &str) -> Option<String> {
        let mut state = self.state.borrow_mut();
        self.push_value(key, &mut state); // S: D d <value>
        let rv = if state.is_string(-1) {
            Some(state.check_string(-1).to_string())
        } else {
            None
        };
        state.pop(2);
        rv
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        let mut state = self.state.borrow_mut();
        self.push_value(key, &mut state); // S: D d <value>
        let rv = if state.is_integer(-1) {
            Some(state.check_integer(-1))
        } else {
            None
        };
        state.pop(2);
        rv
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        let mut state = self.state.borrow_mut();
        self.push_value(key, &mut state); // S: D d <value>
        let rv = if state.is_bool(-1) {
            Some(state.to_bool(-1))
        } else {
            None
        };
        state.pop(2);
        rv
    }
}

impl Task for LuaTask {
    fn name(&self) -> String {
        self.get_string("name").unwrap()
    }

    fn should_run(&self, path: &path::Path) -> bool {
        let mut state = self.state.borrow_mut();
        self.push_value("should_run", &mut state);
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
        if let Some(delay_ms) = self.get_int("start_delay") {
            return time::Duration::milliseconds(delay_ms);
        }
        time::Duration::milliseconds(50)
    }
}

impl ShellTask for LuaTask {
    fn command(&self) -> String {
        self.get_string("command").unwrap()
    }

    fn redirect_stdout(&self) -> Option<path::PathBuf> {
        if let Some(redirect_stdout) = self.get_string("redirect_stdout") {
            return Some(path::PathBuf::from(redirect_stdout));
        }
        None
    }

    fn work_directory(&self) -> Option<path::PathBuf> {
        if let Some(work_directory) = self.get_string("work_directory") {
            return Some(path::PathBuf::from(work_directory));
        }
        None
    }

    fn redirect_stderr(&self) -> Option<path::PathBuf> {
        if let Some(redirect_stderr) = self.get_string("redirect_stderr") {
            return Some(path::PathBuf::from(redirect_stderr));
        }
        None
    }

    fn supress_stderr(&self) -> bool {
        if let Some(redirect_stderr) = self.get_bool("supress_stderr") {
            return redirect_stderr;
        }
        false
    }

    fn supress_stdout(&self) -> bool {
        if let Some(redirect_stdout) = self.get_bool("supress_stdout") {
            return redirect_stdout;
        }
        false
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
        },
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
        },
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
        },
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

// TODO(sirver): error handling
pub fn run_file(path: &path::Path) -> Vec<Box<Task>> {
  let mut state = lua::State::new();
  state.open_libs();

  let rv = state.do_file(&path.to_string_lossy());
  if rv != lua::ThreadStatus::Ok {
      panic!("Error: {:?}", rv);
  }

  inject_path_functions(&mut state);

  let mut tasks = Vec::new();
  let state_rc = Rc::new(RefCell::new(state));
  let mut state = state_rc.borrow_mut();

  state.push_nil(); // S: D nil
  while state.next(-2) {
      let key = state.check_integer(-2); // S: D key value
      state.pop(1); // S: D key
      tasks.push(
          Box::new(LuaTask::new(state_rc.clone(), key)) as Box<Task>);
  }
  // S: D
  tasks
}
