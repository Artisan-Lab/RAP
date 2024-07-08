#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_metadata;
extern crate rustc_data_structures;
extern crate rustc_session;

#[macro_use]
extern crate log as rust_log;

use rustc_driver::{Compilation, Callbacks};
use rustc_interface::{interface::Compiler, Queries};
use rustc_session::config::ErrorOutputType;
use rustc_session::EarlyErrorHandler;
use rustc_middle::util::Providers;
use rustc_interface::Config;
use rustc_session::search_paths::PathKind;
use rustc_data_structures::sync::Lrc;

use std::env;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use rap::{RapConfig, compile_time_sysroot, RAP_DEFAULT_ARGS, start_analyzer};
use rap::analysis::rcanary::flow_analysis::{IcxSliceDisplay, Z3GoalDisplay};
use rap::analysis::rcanary::type_analysis::AdtOwnerDisplay;
use rap::components::{display::MirDisplay, log::Verbosity};
use rap::rap_info;

#[derive(Copy, Clone)]
struct RapCompilerCalls {
    rap_config: RapConfig,
}

impl Default for RapCompilerCalls {
    fn default() -> Self { Self { rap_config: RapConfig::default() } }
}

impl Display for RapCompilerCalls {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,"{}", 0,)
    }
}

impl Callbacks for RapCompilerCalls {
    fn config(&mut self, config: &mut Config) {
        config.override_queries = Some(|_, providers| {
            providers.extern_queries.used_crate_source = |tcx, cnum| {
                let mut providers = Providers::default();
                rustc_metadata::provide(&mut providers);
                let mut crate_source = (providers.extern_queries.used_crate_source)(tcx, cnum);
                // HACK: rustc will emit "crate ... required to be available in rlib format, but
                // was not found in this form" errors once we use `tcx.dependency_formats()` if
                // there's no rlib provided, so setting a dummy path here to workaround those errors.
                Lrc::make_mut(&mut crate_source).rlib = Some((PathBuf::new(), PathKind::All));
                crate_source
            };
        });
    }

    fn after_analysis<'tcx>(
        &mut self,
        compiler: &Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        compiler.session().abort_if_errors();
        Verbosity::init_rap_log_system_with_verbosity(self.rap_config.verbose()).expect("Failed to set up RAP log system");

        rap_info!("Start RAP with the after_analysis function of Callbacks");
        queries.global_ctxt().unwrap().enter(
            |tcx| start_analyzer(tcx, self.rap_config)
        );
        rap_info!("Stop RAP");

        compiler.session().abort_if_errors();
        Compilation::Continue
    }
}

impl RapCompilerCalls {
    #[allow(dead_code)]
    fn new(rap_config: RapConfig) -> Self { Self {rap_config} }
}

struct RapArgs {
    rap_cc: RapCompilerCalls,
    args: Vec<String>,
}

impl Default for RapArgs {
    fn default() -> Self {
        Self {
            rap_cc: RapCompilerCalls::default(),
            args: vec![],
        }
    }
}

impl Display for RapArgs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} Whole Args: {:?}", self.rap_cc, self.args)
    }
}

impl RapArgs {

    pub fn set_mir_display(&mut self) {self.rap_cc.rap_config.set_mir_display(MirDisplay::Simple); }

    pub fn set_mir_display_verbose(&mut self) { self.rap_cc.rap_config.set_mir_display(MirDisplay::Verobse); }

    pub fn enable_rcanary(&mut self) { self.rap_cc.rap_config.enable_rcanary(); }

    pub fn set_adt_display_verbose(&mut self) {
        if self.rap_cc.rap_config.is_rcanary_enabled() {
            self.rap_cc.rap_config.set_adt_display(AdtOwnerDisplay::Verbose);
        }
    }

    pub fn set_z3_goal_display_verbose(&mut self) {
        if self.rap_cc.rap_config.is_rcanary_enabled() {
            self.rap_cc.rap_config.set_z3_goal_display(Z3GoalDisplay::Verbose);
        }
    }

    pub fn set_icx_slice_display(&mut self) {
        if self.rap_cc.rap_config.is_rcanary_enabled() {
            self.rap_cc.rap_config.set_icx_slice_display(IcxSliceDisplay::Verbose);
        }
    }

    pub fn push_args(&mut self, arg: String) { self.args.push(arg); }

    pub fn splice_args(&mut self) {
        self.args.splice(1..1, RAP_DEFAULT_ARGS.iter().map(ToString::to_string));
    }
}

fn config_parse() -> RapArgs {
    let mut rap_args = RapArgs::default();
    //FIXME: logging doesn't work here;
    //rap_info!("rap received arguments{:#?}", env::args());
    for arg in env::args() {
        match arg.as_str() {
            "-F" | "-uaf" => {}, //FIXME: println!("dummy front end for safedrop; this will be captured by the compiler."),
            "-M" | "-mleak" => rap_args.enable_rcanary(),
            "-adt" => rap_args.set_adt_display_verbose(),
            "-z3" => rap_args.set_z3_goal_display_verbose(),
            "-meta" => rap_args.set_icx_slice_display(),
            "-mir" => rap_args.set_mir_display(),
            "-mir=verbose" => rap_args.set_mir_display_verbose(),
            _ => rap_args.push_args(arg),
        }
    }
    rap_args
}

/// Execute a compiler with the given CLI arguments and callbacks.
fn run_complier(rap_args: &mut RapArgs) -> i32 {
    // Make sure we use the right default sysroot. The default sysroot is wrong,
    // because `get_or_default_sysroot` in `librustc_session` bases that on `current_exe`.
    //
    // Make sure we always call `compile_time_sysroot` as that also does some sanity-checks
    // of the environment we were built in.
    // FIXME: Ideally we'd turn a bad build env into a compile-time error via CTFE or so.
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

    let run_compiler = rustc_driver::RunCompiler::new(&rap_args.args, &mut rap_args.rap_cc);
    let exit_code = rustc_driver::catch_with_exit_code(move || run_compiler.run());

    if option_env!("RAP_DEBUG").is_some() {
        rap_info!("The arg for compilation is {:?}", rap_final_args);
    }

    exit_code
}

const BUG_REPORT_URL: &str = "https://github.com/";

fn main() {
    rap_info!("Enter RAP main.");
    // Instals a panic hook that will print the ICE message on unexpected panics.
    let handler = EarlyErrorHandler::new(ErrorOutputType::default());
    rustc_driver::init_rustc_env_logger(&handler);
    rustc_driver::install_ice_hook(BUG_REPORT_URL, |_| ());

    // Parse the config and arguments from env.
    let mut rap_args = config_parse();

    debug!("RAP-Args: {}", &rap_args);

    let exit_code = run_complier(&mut rap_args);
    std::process::exit(exit_code)
}
