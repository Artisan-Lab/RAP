use crate::args;
use rap::utils::log::rap_error_and_exit;
use std::{process::Command, time::Duration};
use wait_timeout::ChildExt;

pub fn run_cargo_check() {
    let [rap_args, cargo_args] = crate::args::rap_and_cargo_args();
    rap_debug!("rap_args={rap_args:?}\tcargo_args={cargo_args:?}");

    /*Here we prepare the cargo command as cargo check, which is similar to build, but much faster*/
    let mut cmd = Command::new("cargo");
    cmd.arg("check");

    /* set the target as a filter for phase_rustc_rap */
    cmd.args(cargo_args);

    // Serialize the remaining args into a special environment variable.
    // This will be read by `phase_rustc_rap` when we go to invoke
    // our actual target crate (the binary or the test we are running).

    cmd.env(
        "RAP_ARGS",
        serde_json::to_string(rap_args).expect("Failed to serialize args."),
    );

    // Invoke actual cargo for the job, but with different flags.
    let cargo_rap_path = args::current_exe_path();
    cmd.env("RUSTC_WRAPPER", cargo_rap_path);

    rap_debug!("Command is: {:?}.", cmd);

    let mut child = cmd.spawn().expect("Could not run cargo check.");
    match child
        .wait_timeout(Duration::from_secs(60 * 60)) // 1 hour timeout
        .expect("Failed to wait for subprocess.")
    {
        Some(status) => {
            if !status.success() {
                rap_error_and_exit("Finished with non-zero exit code.");
            }
        }
        None => {
            child.kill().expect("Failed to kill subprocess.");
            child.wait().expect("Failed to wait for subprocess.");
            rap_error_and_exit("Process killed due to timeout.");
        }
    };
}
