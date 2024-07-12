#![feature(rustc_private)]
#![feature(control_flow_enum)]
#![feature(box_patterns)]

pub mod analysis;
pub mod components;

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_metadata;
extern crate rustc_data_structures;
extern crate rustc_session;
extern crate rustc_hir;
extern crate rustc_span;
extern crate rustc_index;
extern crate rustc_target;
extern crate rustc_abi;

extern crate serde;
extern crate serde_json;

extern crate serde_derive;
extern crate core;

use rustc_middle::ty::TyCtxt;
use rustc_driver::{Compilation, Callbacks};
use rustc_interface::{interface::Compiler, Queries};
use rustc_middle::util::Providers;
use rustc_interface::Config;
use rustc_session::search_paths::PathKind;
use rustc_data_structures::sync::Lrc;

//use std::fmt::{Display, Formatter};
use crate::components::log::Verbosity;
use crate::components::context::RapGlobalCtxt;
use crate::components::display::MirDisplay;
use crate::analysis::rcanary::flow_analysis::{FlowAnalysis, IcxSliceDisplay, Z3GoalDisplay};
use crate::analysis::rcanary::type_analysis::{TypeAnalysis, AdtOwnerDisplay};

use std::path::PathBuf;

// Insert rustc arguments at the beginning of the argument list that RAP wants to be
// set per default, for maximal validation power.
pub static RAP_DEFAULT_ARGS: &[&str] =
    &["-Zalways-encode-mir", "-Zmir-opt-level=0", "--cfg=rap"];
pub static RAP_ROOT:&str = "/tmp/rap";

pub type Elapsed = (i64, i64);

#[derive(Debug, Copy, Clone, Hash, Default)]
struct SafeDrop(bool);

#[derive(Debug, Copy, Clone, Hash)]
struct RCanary {
    enable: bool,
    adt_display: AdtOwnerDisplay,
    z3_goal_display: Z3GoalDisplay,
    icx_slice_display: IcxSliceDisplay,
}

impl Default for RCanary {
    fn default() -> Self {
        Self {
            enable: false,
            adt_display: AdtOwnerDisplay::Disabled,
            z3_goal_display: Z3GoalDisplay::Disabled,
            icx_slice_display: IcxSliceDisplay::Disabled,
        }
    }
}

#[derive(Debug, Copy, Clone, Hash)]
pub struct RapConfig {
    verbose: Verbosity,
    mir_display: MirDisplay,
    rcanary: RCanary,
    safedrop: SafeDrop,
}

impl Default for RapConfig {
    fn default() -> Self {
        Self {
            verbose: Verbosity::Info,
            mir_display: MirDisplay::Disabled,
            rcanary: RCanary::default(),
            safedrop: SafeDrop::default(),
        }
    }
}

impl Callbacks for RapConfig {
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

        rap_info!("Execute after_analysis() of compiler callbacks");
        queries.global_ctxt().unwrap().enter(
            |tcx| start_analyzer(tcx, *self)
        );
        rap_info!("analysis done");

        compiler.session().abort_if_errors();
        Compilation::Continue
    }
}
impl RapConfig {

    pub fn verbose(&self) -> Verbosity { self.verbose }

    pub fn set_verbose(&mut self, verbose: Verbosity) { self.verbose = verbose; }

    pub fn mir_display(&self) -> MirDisplay { self.mir_display }

    pub fn set_mir_display(&mut self, mir_display: MirDisplay) { self.mir_display = mir_display; }

    pub fn adt_display(&self) -> AdtOwnerDisplay { self.rcanary.adt_display }

    pub fn enable_safedrop(&mut self) { self.safedrop.0 = true; }

    pub fn is_safedrop_enabled(&self) -> bool { self.safedrop.0 }

    pub fn enable_rcanary(&mut self) { self.rcanary.enable = true; }

    pub fn is_rcanary_enabled(&self) -> bool { self.rcanary.enable }

    pub fn set_adt_display(&mut self, adt_display: AdtOwnerDisplay) { self.rcanary.adt_display = adt_display; }

    pub fn z3_goal_display(&self) -> Z3GoalDisplay { self.rcanary.z3_goal_display }

    pub fn set_z3_goal_display(&mut self, z3_goal_display: Z3GoalDisplay) { self.rcanary.z3_goal_display = z3_goal_display; }

    pub fn icx_slice_display(&self) -> IcxSliceDisplay { self.rcanary.icx_slice_display }

    pub fn set_icx_slice_display(&mut self, icx_slice_display: IcxSliceDisplay) { self.rcanary.icx_slice_display = icx_slice_display; }

}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum RapPhase {
    Cleanup,
    Cargo,
    Rustc,
    LLVM, // unimplemented yet
}

/// Returns the "default sysroot" that RAP will use if no `--sysroot` flag is set.
/// Should be a compile-time constant.
pub fn compile_time_sysroot() -> Option<String> {
    // Optionally inspects an environment variable at compile time.
    if option_env!("RUSTC_STAGE").is_some() {
        // This is being built as part of rustc, and gets shipped with rustup.
        // We can rely on the sysroot computation in rustc.
        return None;
    }

    // For builds outside rustc, we need to ensure that we got a sysroot
    // that gets used as a default.  The sysroot computation in librustc_session would
    // end up somewhere in the build dir (see `get_or_default_sysroot`).
    // Taken from PR <https://github.com/Manishearth/rust-clippy/pull/911>.
    let home = option_env!("RUSTUP_HOME").or(option_env!("MULTIRUST_HOME"));
    let toolchain = option_env!("RUSTUP_TOOLCHAIN").or(option_env!("MULTIRUST_TOOLCHAIN"));
    let env = if home.is_some() && toolchain.is_some() {
         format!("{}/toolchains/{}", home.unwrap(), toolchain.unwrap())
    } else {
        option_env!("RUST_SYSROOT")
            .expect("To build RAP without rustup, set the `RUST_SYSROOT` env var at build time")
            .to_string()
    };

    Some(env)
}

fn run_analyzer<F, R>(name: &str, func: F) -> R
    where F: FnOnce() -> R
{
    rap_info!("{} Start", name);
    let res = func();
    rap_info!("{} Done", name);
    res
}

pub fn start_analyzer(tcx: TyCtxt, config: RapConfig) {
    let rcx_boxed = Box::new(RapGlobalCtxt::new(tcx, config));
    let rcx = Box::leak(rcx_boxed);

    if config.is_rcanary_enabled() {
        run_analyzer(
            "Type Analysis",
            ||
                TypeAnalysis::new(rcx).start()
        );

        run_analyzer(
            "Flow Analysis",
            ||
                FlowAnalysis::new(rcx).start()
        );
    }
}
