return {
   {
      name = "Cargo build",
      command = "cargo build --color always",
      start_delay = 250,
      redirect_stdout = "/tmp/build.out",
      redirect_stderr = "/tmp/build.err",
      should_run = function(path)
         return path:ext() == "rs" or path:ext() == "toml" 
      end,
   },
}
