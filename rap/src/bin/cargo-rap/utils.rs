use crate::args;
use std::{
    env,
    path::PathBuf,
    process::{self, Command},
};

fn find_rap() -> PathBuf {
    let mut path = env::current_exe().expect("Current executable path invalid.");
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
    cmd.args(args::skip2());
    run_cmd(cmd);
}

pub fn run_rap() {
    let mut cmd = Command::new(find_rap());
    cmd.args(args::skip2());
    let magic = env::var("RAP_ARGS").expect("Missing RAP_ARGS.");
    let rap_args: Vec<String> =
        serde_json::from_str(&magic).expect("Failed to deserialize RAP_ARGS.");
    cmd.args(rap_args);
    run_cmd(cmd);
}
