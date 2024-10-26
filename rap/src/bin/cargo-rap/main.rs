/*
    This is a cargo program to start RAP.
    The file references the cargo file for Miri: https://github.com/rust-lang/miri/blob/master/cargo-miri/src/main.rs
*/

#[macro_use]
extern crate rap;

use rap::utils::log::{init_log, rap_error_and_exit};

mod args;
mod help;

mod utils;
use crate::utils::*;

mod target;
use target::*;

fn phase_cargo_rap() {
    rap_info!("Start cargo-rap");

    // here we skip two args: cargo rap
    let Some(arg) = args::get_arg(2) else {
        rap_error!("Expect command: e.g., `cargo rap -help`.");
        return;
    };
    match arg {
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

    run_cargo_check();
}

fn phase_rustc_wrapper() {
    rap_debug!("Launch cargo-rap again triggered by cargo check.");

    let is_direct = args::is_current_compile_crate();
    if is_direct {
        run_rap();
        return;
    }

    run_rustc();
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

    match args::get_arg(1).unwrap() {
        s if s.ends_with("rap") => phase_cargo_rap(),
        s if s.ends_with("rustc") => phase_rustc_wrapper(),
        _ => {
            rap_error_and_exit("rap must be called with either `rap` or `rustc` as first argument.")
        }
    }
}
