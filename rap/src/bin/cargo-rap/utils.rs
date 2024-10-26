use rap::{rap_debug, rap_error, rap_info};
use std::{
    path::PathBuf,
    process::{self, Command},
};

pub fn find_rap() -> PathBuf {
    let mut path = std::env::current_exe().expect("Current executable path invalid.");
    path.set_file_name("rap");
    path
}

pub fn run_cmd(mut cmd: Command) {
    rap_debug!("Command is: {:?}.", cmd);
    match cmd.status() {
        Ok(status) => {
            if !status.success() {
                process::exit(status.code().unwrap());
            }
        }
        Err(err) => panic!("Error in running {:?} {}.", cmd, err),
    }
}
