return {
   {
      should_run = function(path)
         return path:ext() == "rs" or path:ext() == "toml" 
      end,
      redirect_stdout = "/tmp/cargo.out",
      redirect_stderr = "/tmp/cargo.err",
      start_delay = 50,
      commands = {
         {
            name = "Cargo build (release)",
            command = "cargo build --release --color always",
         },
      },
   },
}
