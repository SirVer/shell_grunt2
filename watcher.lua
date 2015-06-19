task = {
   name = "Cargo build",
   command = "cargo build",
   start_delay = 250,
   redirect_stdout = "/tmp/build.out",
   redirect_stderr = "/tmp/build.err",
   should_run = function(path)
      return path:ext() == "rs"
   end,
}

return { task }
