/*
 * SPDX-License-Identifier: Apache-2.0
 */

#[allow(unused_imports)]
use fs4::FileExt;
use std::fs::File;
use std::path::Path;

pub struct SingleInstance {
    _lock_file: File,
}

impl SingleInstance {
    pub fn new(name: &str, socket_dir: &str) -> Result<Self, String> {
        let lock_path = Path::new(socket_dir).join(format!("{}.lock", name));
        
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .map_err(|e| format!("Failed to open lock file {:?}: {}", lock_path, e))?;

        match file.try_lock() {
            Ok(_) => Ok(Self { _lock_file: file }),
            Err(_) => Err(format!("Another instance of {} is already running", name)),
        }
    }
}
