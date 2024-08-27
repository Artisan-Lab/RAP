#![feature(rustc_private)]
#![feature(control_flow_enum)]
#![feature(box_patterns)]

pub mod analysis;
pub mod utils;

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_metadata;
extern crate rustc_data_structures;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;
extern crate rustc_hir;

use rustc_middle::ty::TyCtxt;
use rustc_driver::{Compilation, Callbacks};
use rustc_interface::{Config, Queries};
use rustc_interface::interface::Compiler;
use rustc_middle::util::Providers;
use rustc_session::search_paths::PathKind;
use rustc_data_structures::sync::Lrc;
use std::path::PathBuf;
use analysis::rcanary::rCanary;
use analysis::unsafety_isolation::UnsafetyIsolationCheck;
use analysis::safedrop::SafeDrop;
use analysis::core::control_flow::callgraph::CallGraph;
use analysis::utils::show_mir::ShowMir;


// Insert rustc arguments at the beginning of the argument list that RAP wants to be
// set per default, for maximal validation power.
pub static RAP_DEFAULT_ARGS: &[&str] =
    &["-Zalways-encode-mir", "-Zmir-opt-level=0", "--cfg=rap"];

pub type Elapsed = (i64, i64);

#[derive(Debug, Copy, Clone, Hash)]
pub struct RapCallback {
    rcanary: bool,
    safedrop: usize,
    unsafety_isolation: bool,
    callgraph: bool,
    show_mir: bool,
}

impl Default for RapCallback {
    fn default() -> Self {
        Self {
            rcanary: false,
            safedrop: 0,
            unsafety_isolation: false,
            callgraph: false,
            show_mir: false,
        }
    }
}

impl Callbacks for RapCallback {
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

impl RapCallback {
    pub fn enable_rcanary(&mut self) { 
	    self.rcanary = true; 
    }

    pub fn is_rcanary_enabled(&self) -> bool { 
	    self.rcanary 
    }

    pub fn enable_safedrop(&mut self, x:usize) { 
	    self.safedrop = x; // 1: backend version; 2: frontend version 
    }

    pub fn is_safedrop_enabled(&self) -> usize { 
	    self.safedrop
    }

    pub fn enable_unsafety_isolation(&mut self) { 
        self.unsafety_isolation = true; 
    }
    
    pub fn is_unsafety_isolation_enabled(&self) -> bool { 
        self.unsafety_isolation 
    }

    pub fn enable_callgraph(&mut self) { 
	self.callgraph = true; 
    }

    pub fn is_callgraph_enabled(&self) -> bool { 
	self.callgraph 
    }

    pub fn enable_show_mir(&mut self) { 
	self.show_mir = true; 
    }

    pub fn is_show_mir_enabled(&self) -> bool { 
	self.show_mir 
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

pub fn start_analyzer(tcx: TyCtxt, callback: RapCallback) {
    if callback.is_rcanary_enabled() {
	    rCanary::new(tcx).start()
    }

    /* legacy: Backend version */
    if callback.is_safedrop_enabled() == 1 {
        tcx.hir().par_body_owners(|def_id| tcx.ensure().query_safedrop(def_id));
    }

    /* Frontend Version */
    if callback.is_safedrop_enabled() == 2 {
        SafeDrop::new(tcx).start();
    }

    if callback.is_unsafety_isolation_enabled() {
        UnsafetyIsolationCheck::new(tcx).start();
    }

    if callback.is_callgraph_enabled() {
        CallGraph::new(tcx).start();
    }

    if callback.is_show_mir_enabled() {
        ShowMir::new(tcx).start();
    }
}

