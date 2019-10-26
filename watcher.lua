return {
   {
      should_run = function(path)
         if path:find("target") ~= nil then
            return false
         end
         return path:ext() == "rs" or path:ext() == "toml" 
      end,
      redirect_stdout = "/tmp/cargo.out",
      redirect_stderr = "/tmp/cargo.err",
      start_delay = 50,
      commands = {
         {
            name = "Cargo check",
            command = "cargo check --color always",
         },
         {
            name = "Cargo build (release)",
            command = "cargo build --release --color always",
         },
         {
            name = "Cargo clippy",
            command = "cargo clippy --color always",
         },
      },
   },
}
