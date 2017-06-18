return {
   {
      name = "Main chain.",
      should_run = function(path)
         return path:ext() == "rs" or path:ext() == "toml" 
      end,
      redirect_stdout = "/tmp/build.out",
      redirect_stderr = "/tmp/build.err",
      start_delay = 50,
            -- name = "Cargo check",
            -- command = "cargo check --color always",

      commands = {
         {
            name = "Cargo check",
            command = "cargo check --color always",
         },
         {
            name = "Cargo build (debug)",
            command = "cargo build --color always",
         },
         {
            name = "Cargo build (release)",
            command = "cargo build --release --color always",
         },
      },
   },
}
