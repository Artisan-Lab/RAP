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
extern crate rustc_span;
extern crate rustc_target;

use rustc_middle::ty::TyCtxt;
use rustc_driver::{Compilation, Callbacks};
use rustc_interface::{interface::Compiler, Queries};
use rustc_middle::util::Providers;
use rustc_interface::Config;
use rustc_session::search_paths::PathKind;
use rustc_data_structures::sync::Lrc;

use crate::components::context::RapGlobalCtxt;
use crate::analysis::rcanary::flow_analysis::{FlowAnalysis};
use crate::analysis::rcanary::type_analysis::{TypeAnalysis};

use std::path::PathBuf;

// Insert rustc arguments at the beginning of the argument list that RAP wants to be
// set per default, for maximal validation power.
pub static RAP_DEFAULT_ARGS: &[&str] =
    &["-Zalways-encode-mir", "-Zmir-opt-level=0", "--cfg=rap"];
//pub static RAP_ROOT:&str = "/tmp/rap";

pub type Elapsed = (i64, i64);

#[derive(Debug, Copy, Clone, Hash, Default)]
struct SafeDrop(bool);

#[derive(Debug, Copy, Clone, Hash)]
struct RCanary {
    enable: bool,
}

impl Default for RCanary {
    fn default() -> Self {
        Self {
            enable: false,
        }
    }
}

#[derive(Debug, Copy, Clone, Hash)]
pub struct RapConfig {
    rcanary: RCanary,
}

impl Default for RapConfig {
    fn default() -> Self {
        Self {
            rcanary: RCanary::default(),
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
    pub fn enable_rcanary(&mut self) { 
	self.rcanary.enable = true; 
    }

    pub fn is_rcanary_enabled(&self) -> bool { 
	self.rcanary.enable 
    }
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

pub fn start_analyzer(tcx: TyCtxt, config: RapConfig) {
    let rcx_boxed = Box::new(RapGlobalCtxt::new(tcx, config));
    let rcx = Box::leak(rcx_boxed);

    if config.is_rcanary_enabled() {
        TypeAnalysis::new(rcx).start();
        FlowAnalysis::new(rcx).start();
    }
}
