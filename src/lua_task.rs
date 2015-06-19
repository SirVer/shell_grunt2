extern crate lua;
extern crate libc;

use self::lua::ffi::lua_State;
use std::path;
use std::sync::{Arc, Mutex};
use task::Task;


struct LuaTask {
    state: Arc<Mutex<lua::State>>,
    key: i64,
}

// NOCOM(#sirver): rethink that.
unsafe impl Sync for LuaTask {}

impl LuaTask {
    fn new(state: Arc<Mutex<lua::State>>, key: i64) -> LuaTask {
        LuaTask {
            state: state,
            key: key,
        }
    }

    fn get_string(&self, key: &str) -> String {
        // S: D
        let mut state = self.state.lock().unwrap();
        state.push_integer(self.key); // S: D key
        state.get_table(1); // S: D d
        state.push_string(key); // S: D d <key>
        state.get_table(-2); // S: D d <value>
        let s = state.check_string(-1);
        state.pop(2);
        s
    }

}

impl Task for LuaTask {
    fn name(&self) -> String {
        self.get_string("name")
    }

    fn command(&self) -> String {
        self.get_string("command")
    }

    fn should_run(&self, path: &path::Path) -> bool {
        let mut state = self.state.lock().unwrap();
        state.push_integer(self.key); // S: D key
        state.get_table(1); // S: D d
        state.push_string("should_run"); // S: D d <key>
        state.get_table(-2); // S: D d <value>
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
