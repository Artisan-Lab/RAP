#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_session;

use rustc_session::config::ErrorOutputType;
use rustc_session::EarlyErrorHandler;

use std::env;
use std::fmt::{Display, Formatter};

use rap::{RapConfig, compile_time_sysroot, RAP_DEFAULT_ARGS};
use rap::analysis::rcanary::flow_analysis::{IcxSliceDisplay, Z3GoalDisplay};
use rap::analysis::rcanary::type_analysis::AdtOwnerDisplay;
use rap::components::{display::MirDisplay, log::Verbosity};
use rap::{rap_info, rap_debug};


struct RapArgs {
    config: RapConfig,
    args: Vec<String>,
}

impl Default for RapArgs {
    fn default() -> Self {
        Self {
            config: RapConfig::default(),
            args: vec![],
        }
    }
}

impl Display for RapArgs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} Whole Args: {:?}", self.config, self.args)
    }
}

impl RapArgs {

    pub fn set_mir_display(&mut self) {
	self.config.set_mir_display(MirDisplay::Simple); 
    }

    pub fn set_mir_display_verbose(&mut self) { 
	self.config.set_mir_display(MirDisplay::Verobse); 
    }

    pub fn enable_rcanary(&mut self) { 
	self.config.enable_rcanary(); 
    }

    pub fn set_adt_display(&mut self) {
        self.config.set_adt_display(AdtOwnerDisplay::Verbose);
    }

    pub fn set_z3_display(&mut self) {
        self.config.set_z3_display(Z3GoalDisplay::Verbose);
    }

    pub fn set_icx_slice_display(&mut self) {
        self.config.set_icx_slice_display(IcxSliceDisplay::Verbose);
    }

    pub fn push_args(&mut self, arg: String) { 
	self.args.push(arg); 
    }

    pub fn splice_args(&mut self) {
        self.args.splice(1..1, RAP_DEFAULT_ARGS.iter().map(ToString::to_string));
    }
}


fn run_complier(rap_args: &mut RapArgs) -> i32 {
    if let Some(sysroot) = compile_time_sysroot() {
        let sysroot_flag = "--sysroot";
        if !rap_args.args.iter().any(|e| e == sysroot_flag) {
            // We need to overwrite the default that librustc_session would compute.
            rap_args.push_args(sysroot_flag.to_owned());
            rap_args.push_args(sysroot);
        }
    }
    // Finally, add the default flags all the way in the beginning, but after the binary name.
    rap_args.splice_args();

    let rap_final_args = rap_args.args.clone();

    let handler = EarlyErrorHandler::new(ErrorOutputType::default());
    rustc_driver::init_rustc_env_logger(&handler);
    rustc_driver::install_ice_hook("bug_report_url", |_|());

    let run_compiler = rustc_driver::RunCompiler::new(&rap_args.args, &mut rap_args.config);
    let exit_code = rustc_driver::catch_with_exit_code(move || run_compiler.run());

    rap_debug!("The arg for compilation is {:?}", rap_final_args);

    exit_code
}

fn main() {
    //Verbosity::init_log(Verbosity::Debug).expect("Failed to init log");
    Verbosity::init_log(Verbosity::Info).expect("Failed to init log");
    rap_info!("Enter rap.");

    // Parse the arguments from env.
    let mut rap_args = RapArgs::default();
    rap_debug!("rap received arguments{:#?}", env::args());
    for arg in env::args() {
        match arg.as_str() {
            "-F" | "-uaf" => {}, //FIXME: println!("dummy front end for safedrop; this will be captured by the compiler."),
            "-M" | "-mleak" => rap_args.enable_rcanary(),
            "-adt" => rap_args.set_adt_display(),
            "-z3" => rap_args.set_z3_display(),
            "-meta" => rap_args.set_icx_slice_display(),
            "-mir" => rap_args.set_mir_display(),
            "-mir=verbose" => rap_args.set_mir_display_verbose(),
            _ => rap_args.push_args(arg),
        }
    }
    rap_debug!("RAP-Args: {}", &rap_args);

    let exit_code = run_complier(&mut rap_args);
    std::process::exit(exit_code)
}
