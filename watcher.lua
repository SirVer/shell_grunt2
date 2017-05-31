return {
   {
      name = "Cargo check",
      command = "cargo check --color always",
      start_delay = 50,
      redirect_stdout = "/tmp/build.out",
      redirect_stderr = "/tmp/build.err",
      should_run = function(path)
         return path:ext() == "rs" or path:ext() == "toml" 
      end,
   },
}
