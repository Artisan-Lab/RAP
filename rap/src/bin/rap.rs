#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_session;

use rustc_session::config::ErrorOutputType;
use rustc_session::EarlyErrorHandler;
use std::env;
use rap::{RapCallback, compile_time_sysroot, RAP_DEFAULT_ARGS};
use rap::utils::log::Verbosity;
use rap::{rap_info, rap_debug};

fn run_complier(args: &mut Vec<String>, callback: &mut RapCallback) -> i32 {
    if let Some(sysroot) = compile_time_sysroot() {
        let sysroot_flag = "--sysroot";
        if !args.iter().any(|e| e == sysroot_flag) {
            // We need to overwrite the default that librustc_session would compute.
            args.push(sysroot_flag.to_owned());
            args.push(sysroot);
        }
    }
    // Finally, add the default flags all the way in the beginning, but after the binary name.
    args.splice(1..1, RAP_DEFAULT_ARGS.iter().map(ToString::to_string));

    let handler = EarlyErrorHandler::new(ErrorOutputType::default());
    rustc_driver::init_rustc_env_logger(&handler);
    rustc_driver::install_ice_hook("bug_report_url", |_|());

    let run_compiler = rustc_driver::RunCompiler::new(&args, callback);
    let exit_code = rustc_driver::catch_with_exit_code(move || run_compiler.run());
    rap_debug!("The arg for compilation is {:?}", args);

    exit_code
}

fn main() {
    //Verbosity::init_log(Verbosity::Debug).expect("Failed to init log");
    Verbosity::init_log(Verbosity::Info).expect("Failed to init log");
    rap_info!("Enter rap.");

    // Parse the arguments from env.
    let mut args = vec![];
    let mut compiler = RapCallback::default();
    rap_debug!("rap received arguments{:#?}", env::args());
    for arg in env::args() {
        match arg.as_str() {
            "-F" | "-uaf" => {},
            "-M" | "-mleak" => compiler.enable_rcanary(),
            "-adt" => {},
            "-z3" => {},
            "-meta" => {},
            "-UI" => compiler.enable_unsafety_isolation(),
            "-callgraph" => compiler.enable_callgraph(),
            "-mir" => {},
            _ => args.push(arg),
        }
    }
    rap_debug!("Arguments: {:?}", &args);
    let exit_code = run_complier(&mut args, &mut compiler);
    std::process::exit(exit_code)
}
