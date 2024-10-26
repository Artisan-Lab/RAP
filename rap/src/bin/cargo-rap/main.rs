/*
    This is a cargo program to start RAP.
    The file references the cargo file for Miri: https://github.com/rust-lang/miri/blob/master/cargo-miri/src/main.rs
*/

#[macro_use]
extern crate rap;

use cargo_metadata::{Metadata, MetadataCommand};
use rap::utils::log::{init_log, rap_error_and_exit};
use std::{env, process::Command, time::Duration};
use wait_timeout::ChildExt;

mod args;
mod help;

mod utils;
use crate::utils::*;

mod target_kind;
use target_kind::*;

fn phase_cargo_rap() {
    rap_info!("Start cargo-rap");
    let mut args = env::args().skip(2); // here we skip two tokens: cargo rap
    let Some(arg) = args.next() else {
        rap_error!("Expect command: e.g., `cargo rap -help`.");
        return;
    };
    match arg.as_str() {
        "-V" | "-version" => {
            rap_info!("{}", help::RAP_VERSION);
            return;
        }
        "-H" | "-help" | "--help" => {
            rap_info!("{}", help::RAP_HELP);
            return;
        }
        _ => {}
    }

    let cmd = MetadataCommand::new();
    rap_debug!("Please run `cargo metadata` if this step takes too long");
    let mut metadata = match cmd.exec() {
        // execute command: `cargo metadata'
        Ok(metadata) => metadata,
        Err(e) => rap_error_and_exit(format!("Cannot obtain cargo metadata: {}.", e)),
    };

    let [rap_args, cargo_args] = args::rap_and_cargo_args();
    rap_debug!("rap_args={rap_args:?}\tcargo_args={cargo_args:?}");

    let targets = find_targets(&mut metadata);
    for target in targets {
        /*Here we prepare the cargo command as cargo check, which is similar to build, but much faster*/
        let mut cmd = Command::new("cargo");
        cmd.arg("check");

        /* We only process bin and lib targets, and ignore others */
        if !is_identified_target(&target, &mut cmd) {
            rap_debug!("Ignore the target because it is neither bin or lib.");
            continue;
        }

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
        let cargo_rap_path = env::current_exe().expect("Current executable path is invalid.");
        cmd.env("RUSTC_WRAPPER", &cargo_rap_path);

        rap_debug!("Command is: {:?}.", cmd);
        rap_info!(
            "Running rap for target {}:{}",
            TargetKind::from(&target),
            &target.name
        );

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
}

fn phase_rustc_wrapper() {
    rap_debug!("Launch cargo-rap again triggered by cargo check.");

    let is_direct = args::is_current_compile_crate();
    if is_direct {
        let mut cmd = Command::new(find_rap());
        cmd.args(env::args().skip(2));
        let magic = env::var("RAP_ARGS").expect("Missing RAP_ARGS.");
        let rap_args: Vec<String> =
            serde_json::from_str(&magic).expect("Failed to deserialize RAP_ARGS.");
        cmd.args(rap_args);
        run_cmd(cmd);
        return;
    }

    rap_info!("phase_rustc_wrapper: run rustc");
    let mut cmd = Command::new("rustc");
    cmd.args(env::args().skip(2));
    run_cmd(cmd);
}

fn main() {
    /* This function will be enteredd twice:
       1. When we run `cargo rap ...`, cargo dispatches the execution to cargo-rap.
      In this step, we set RUSTC_WRAPPER to cargo-rap, and execute `cargo check ...` command;
       2. Cargo check actually triggers `path/cargo-rap path/rustc` according to RUSTC_WRAPPER.
          Because RUSTC_WRAPPER is defined, Cargo calls the command: `$RUSTC_WRAPPER path/rustc ...`
    */

    // Init the log_system
    init_log().expect("Failed to init log.");
    rap_debug!("Enter cargo-rap; Received args: {:?}", env::args());

    let first_arg = env::args().nth(1);
    match first_arg.unwrap() {
        s if s.ends_with("rap") => phase_cargo_rap(),
        s if s.ends_with("rustc") => phase_rustc_wrapper(),
        _ => {
            rap_error_and_exit("rap must be called with either `rap` or `rustc` as first argument.")
        }
    }
}
