#![feature(rustc_private)]
#![feature(control_flow_enum)]
#![feature(box_patterns)]
pub mod analysis;
pub mod components;

extern crate rustc_driver;
extern crate rustc_middle;
extern crate rustc_hir;
extern crate rustc_span;
extern crate rustc_index;
extern crate rustc_target;
extern crate rustc_abi;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;
extern crate core;

use rustc_middle::ty::TyCtxt;

use crate::components::grain::RapGrain;
use crate::components::log::Verbosity;
use crate::components::context::RapGlobalCtxt;
use crate::components::display::MirDisplay;
use crate::analysis::rcanary::flow_analysis::{FlowAnalysis, IcxSliceDisplay, Z3GoalDisplay};
use crate::analysis::rcanary::type_analysis::{TypeAnalysis, AdtOwnerDisplay};

// Insert rustc arguments at the beginning of the argument list that RAP wants to be
// set per default, for maximal validation power.
pub static RAP_DEFAULT_ARGS: &[&str] =
    &["-Zalways-encode-mir", "-Zmir-opt-level=0", "--cfg=rap"];
pub static RAP_ROOT:&str = "/tmp/rap";
pub static RAP_LLVM_CACHE:&str = "/tmp/rap/llvm-cache";
pub static RAP_LLVM_IR:&str = "/tmp/rap/llvm-ir";
pub static RAP_LLVM_RES:&str = "/tmp/rap/llvm-res";

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

#[derive(Debug, Copy, Clone, Hash, Default)]
struct HelloWorld {
    front: bool,
    back: bool,
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
    grain: RapGrain,
    verbose: Verbosity,
    mir_display: MirDisplay,
    rcanary: RCanary,
    safedrop: SafeDrop,
    hello_world: HelloWorld,
}

impl Default for RapConfig {
    fn default() -> Self {
        Self {
            grain: RapGrain::Low,
            verbose: Verbosity::Info,
            mir_display: MirDisplay::Disabled,
            rcanary: RCanary::default(),
            safedrop: SafeDrop::default(),
            hello_world: HelloWorld::default(),
        }
    }
}

impl RapConfig {
    pub fn new(
        grain: RapGrain,
        verbose: Verbosity,
        mir_display: MirDisplay,
        rcanary: RCanary,
        safedrop: SafeDrop,
        hello_world: HelloWorld,
    ) -> Self {
        Self {
            grain,
            verbose,
            mir_display,
            rcanary,
            safedrop,
            hello_world,
        }
    }

    pub fn grain(&self) -> RapGrain { self.grain }

    pub fn set_grain(&mut self, grain: RapGrain) { self.grain = grain;}

    pub fn verbose(&self) -> Verbosity { self.verbose }

    pub fn set_verbose(&mut self, verbose: Verbosity) { self.verbose = verbose; }

    pub fn mir_display(&self) -> MirDisplay { self.mir_display }

    pub fn set_mir_display(&mut self, mir_display: MirDisplay) { self.mir_display = mir_display; }

    pub fn adt_display(&self) -> AdtOwnerDisplay { self.rcanary.adt_display }

    pub fn enable_safedrop(&mut self) { self.safedrop.0 = true; }

    pub fn is_safedrop_enabled(&self) -> bool { self.safedrop.0 }

    pub fn enable_rcanary(&mut self) { self.rcanary.enable = true; }

    pub fn is_rcanary_enabled(&self) -> bool { self.rcanary.enable }

    pub fn enable_example_frontend(&mut self) { self.hello_world.front = true; }

    pub fn enable_example_backend(&mut self) { self.hello_world.back = true; }

    pub fn is_example_front_enabled(&self) -> bool { self.hello_world.front }

    pub fn is_example_back_enabled(&self) -> bool { self.hello_world.back }

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
