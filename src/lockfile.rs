use sha1;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Lockfile {
    path: PathBuf,
}

pub struct AlreadyExists(pub PathBuf);

impl Lockfile {
    pub fn new<P: AsRef<Path>>(watcher_file: P) -> Result<Self, AlreadyExists> {
        let canonicalized_path = watcher_file.as_ref().canonicalize().unwrap();
        let path = {
            let mut sha = sha1::Sha1::new();
            sha.update(canonicalized_path.to_string_lossy().as_bytes());
            env::temp_dir().join(sha.digest().to_string())
        };

        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => Ok(Lockfile { path }),
            Err(_) => Err(AlreadyExists(path)),
        }
    }
}

impl Drop for Lockfile {
    fn drop(&mut self) {
        // We must never panic in drop.
        let _ = fs::remove_file(&self.path);
    }
}
