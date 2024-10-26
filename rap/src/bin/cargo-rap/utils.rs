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

pub fn run_rustc() {
    let mut cmd = Command::new("rustc");
    cmd.args(crate::args::rustc());
    run_cmd(cmd);
}
