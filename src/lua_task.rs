extern crate libc;
extern crate lua;
extern crate time;

use self::lua::ffi::lua_State;
use std::path;
use std::sync::{Arc, Mutex};
use task::Task;


struct LuaTask {
    state: Arc<Mutex<lua::State>>,
    key: i64,
}

// TODO(sirver): That is actually a lie I think. I do not know if it is valid to use the same lua
// state in multiple threads - even if mutex protected.
unsafe impl Sync for LuaTask {}

impl LuaTask {
    fn new(state: Arc<Mutex<lua::State>>, key: i64) -> LuaTask {
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
        let mut state = self.state.lock().unwrap();
        self.push_value(key, &mut state); // S: D d <value>
        let rv = if state.is_string(-1) {
            Some(state.check_string(-1))
        } else {
            None
        };
        state.pop(2);
        rv
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        let mut state = self.state.lock().unwrap();
        self.push_value(key, &mut state); // S: D d <value>
        let rv = if state.is_integer(-1) {
            Some(state.check_integer(-1))
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

    fn command(&self) -> String {
        self.get_string("command").unwrap()
    }

    fn should_run(&self, path: &path::Path) -> bool {
        let mut state = self.state.lock().unwrap();
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

    fn redirect_stdout(&self) -> Option<path::PathBuf> {
        if let Some(redirect_stdout) = self.get_string("redirect_stdout") {
            return Some(path::PathBuf::from(redirect_stdout));
        }
        None
    }

}

#[allow(non_snake_case)]
unsafe extern "C" fn basename(L: *mut lua_State) -> libc::c_int {
    let mut state = lua::State::from_ptr(L);
    let string: String = state.check_string(-1);
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
    let string: String = state.check_string(-1);
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
    let string: String = state.check_string(-1);
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

	// Also add in string.bformat to use boost::format instead, so that we get
	// // proper localisation.
	// lua_getglobal(L, "string");  // S: string_lib
	// lua_pushstring(L, "bformat");  // S: string_lib "bformat"
	// lua_pushcfunction(L, &L_string_bformat);  // S: string_lib "bformat" function
	// lua_settable(L, -3);  // S: string_lib
	// lua_pop(L, 1);

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
  let state_arc = Arc::new(Mutex::new(state));
  let mut state = state_arc.lock().unwrap();

  state.push_nil(); // S: D nil
  while state.next(-2) {
      let key = state.check_integer(-2); // S: D key value
      // NOCOM(#sirver): check that it is a table.
      // let value = state.check_table(-1);
      state.pop(1); // S: D key
      tasks.push(
          Box::new(LuaTask::new(state_arc.clone(), key)) as Box<Task>);
  }
  // S: D
  println!("#sirver rv: {:?}", rv);
  tasks
}
