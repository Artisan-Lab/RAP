#![feature(rustc_private)]
#![feature(control_flow_enum)]
#![feature(box_patterns)]

#[macro_use]
pub mod utils;

pub mod analysis;

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;

use analysis::core::alias::mop::MopAlias;
use analysis::core::call_graph::CallGraph;
use analysis::core::dataflow::DataFlow;
use analysis::rcanary::rCanary;
use analysis::safedrop::SafeDrop;
use analysis::senryx::SenryxCheck;
use analysis::unsafety_isolation::{UigInstruction, UnsafetyIsolationCheck};
use analysis::utils::show_mir::ShowMir;
use rustc_data_structures::sync::Lrc;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_interface::{Config, Queries};
use rustc_middle::ty::TyCtxt;
use rustc_middle::util::Providers;
use rustc_session::search_paths::PathKind;
use std::path::PathBuf;

// Insert rustc arguments at the beginning of the argument list that RAP wants to be
// set per default, for maximal validation power.
pub static RAP_DEFAULT_ARGS: &[&str] = &["-Zalways-encode-mir", "-Zmir-opt-level=0", "--cfg=rap"];

pub type Elapsed = (i64, i64);

#[derive(Debug, Copy, Clone, Hash)]
pub struct RapCallback {
    rcanary: bool,
    safedrop: bool,
    senryx: bool,
    unsafety_isolation: usize,
    mop: bool,
    callgraph: bool,
    show_mir: bool,
    dataflow: usize,
}

impl Default for RapCallback {
    fn default() -> Self {
        Self {
            rcanary: false,
            safedrop: false,
            senryx: false,
            unsafety_isolation: 0,
            mop: false,
            callgraph: false,
            show_mir: false,
            dataflow: 0,
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
        _compiler: &Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        rap_info!("Execute after_analysis() of compiler callbacks");
        queries
            .global_ctxt()
            .unwrap()
            .enter(|tcx| start_analyzer(tcx, *self));
        rap_info!("analysis done");
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

    pub fn enable_mop(&mut self) {
        self.mop = true;
    }

    pub fn is_mop_enabled(&self) -> bool {
        self.mop
    }

    pub fn enable_safedrop(&mut self) {
        self.safedrop = true;
    }

    pub fn is_safedrop_enabled(&self) -> bool {
        self.safedrop
    }

    pub fn enable_unsafety_isolation(&mut self, x: usize) {
        self.unsafety_isolation = x;
    }

    pub fn is_unsafety_isolation_enabled(&self) -> usize {
        self.unsafety_isolation
    }

    pub fn enable_senryx(&mut self) {
        self.senryx = true;
    }

    pub fn is_senryx_enabled(&self) -> bool {
        self.senryx
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

    pub fn enable_dataflow(&mut self, x: usize) {
        self.dataflow = x;
    }

    pub fn is_dataflow_enabled(self) -> usize {
        self.dataflow
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
    let _rcanary: Option<rCanary> = if callback.is_rcanary_enabled() {
        let mut rcx = rCanary::new(tcx);
        rcx.start();
        Some(rcx)
    } else {
        None
    };

    if callback.is_mop_enabled() {
        MopAlias::new(tcx).start();
    }

    if callback.is_safedrop_enabled() {
        SafeDrop::new(tcx).start();
    }

    let x = callback.is_unsafety_isolation_enabled();
    match x {
        1 => UnsafetyIsolationCheck::new(tcx).start(UigInstruction::UigCount),
        2 => UnsafetyIsolationCheck::new(tcx).start(UigInstruction::Doc),
        3 => UnsafetyIsolationCheck::new(tcx).start(UigInstruction::Upg),
        4 => UnsafetyIsolationCheck::new(tcx).start(UigInstruction::Ucons),
        _ => {}
    }

    if callback.is_senryx_enabled() {
        SenryxCheck::new(tcx, 2).start();
    }

    if callback.is_show_mir_enabled() {
        ShowMir::new(tcx).start();
    }

    match callback.is_dataflow_enabled() {
        1 => DataFlow::new(tcx, false).start(),
        2 => DataFlow::new(tcx, true).start(),
        _ => {}
    }

    if callback.is_callgraph_enabled() {
        CallGraph::new(tcx).start();
    }
}
