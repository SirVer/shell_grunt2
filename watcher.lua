return {
   {
      should_run = function(path)
         return path:ext() == "rs" or path:ext() == "toml" 
      end,
      redirect_stdout = "/tmp/build.out",
      redirect_stderr = "/tmp/build.err",
      start_delay = 50,
      environment = {
         CARGO_INCREMENTAL = "1",
      },
      commands = {
         {
            name = "Cargo build (release)",
            command = "cargo build --release --color always",
         },
      },
   },
}
