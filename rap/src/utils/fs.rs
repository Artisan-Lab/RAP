use crate::utils::log::rap_error_and_exit;

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use rustc_demangle::try_demangle;

pub fn rap_create_dir<P: AsRef<Path>>(path: P, msg: impl AsRef<str>) {
    if fs::read_dir(&path).is_err() {
        fs::create_dir(path)
            .unwrap_or_else(|e|
                rap_error_and_exit(format!("{}: {}", msg.as_ref(), e))
            );
    }
}

pub fn rap_remove_dir<P: AsRef<Path>>(path: P, msg: impl AsRef<str>) {
    if fs::read_dir(&path).is_ok() {
        fs::remove_dir_all(path)
            .unwrap_or_else(|e|
                rap_error_and_exit(format!("{}: {}", msg.as_ref(), e))
            );
    }
}

pub fn rap_can_read_dir<P: AsRef<Path>>(path: P, msg: impl AsRef<str>) -> bool {
    match fs::read_dir(path) {
        Ok(_) => true,
        Err(e) => rap_error_and_exit(format!("{}: {}", msg.as_ref(), e)),
    }
}

pub fn rap_copy_file<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q, msg: impl AsRef<str>) {
    fs::copy(from, to)
        .unwrap_or_else(|e|
            rap_error_and_exit(format!("{}: {}", msg.as_ref(), e))
        );
}

pub fn rap_create_file<P: AsRef<Path>>(path: P, msg: impl AsRef<str>) -> fs::File {
    match fs::File::create(path) {
        Ok(file) => file,
        Err(e) => rap_error_and_exit(format!("{}: {}", msg.as_ref(), e)),
    }
}

pub fn rap_read<P: AsRef<Path>>(path: P, msg: impl AsRef<str>) -> fs::File {
    match fs::File::open(path) {
        Ok(file) => file,
        Err(e) => rap_error_and_exit(format!("{}: {}", msg.as_ref(), e)),
    }
}

pub fn rap_write(mut file: File, buf: &[u8], msg: impl AsRef<str>) -> usize {
    file.write(buf)
        .unwrap_or_else(|e|
            rap_error_and_exit(format!("{}: {}", msg.as_ref(), e)),
        )
}

pub fn rap_demangle(name: &str) -> String {
    match try_demangle(name) {
        Ok(d) => format!("{:#}", d),
        Err(_) => name.to_string(),
    }
}
