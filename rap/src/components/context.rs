use rustc_middle::ty::TyCtxt;

use crate::{RapCallback, Elapsed};
use crate::analysis::rcanary::flow_analysis::MirGraph;
use crate::analysis::rcanary::type_analysis::AdtOwner;
use std::collections::HashMap;

#[derive(Clone)]
pub struct RapGlobalCtxt<'tcx> {
    tcx: TyCtxt<'tcx>,
    callback: RapCallback,
    adt_owner: AdtOwner,
    mir_graph: MirGraph,
    elapsed: Elapsed,
}

impl<'tcx> RapGlobalCtxt<'tcx> {
    pub fn new(tcx:TyCtxt<'tcx>, callback: RapCallback) -> Self {
        Self {
            tcx,
            callback,
            adt_owner: HashMap::default(),
            mir_graph: HashMap::default(),
            elapsed: (0, 0),
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn callback(&self) -> RapCallback {
        self.callback
    }

    pub fn adt_owner(&self) -> &AdtOwner {
        &self.adt_owner
    }

    pub fn adt_owner_mut(&mut self) -> &mut AdtOwner {
        &mut self.adt_owner
    }

    pub fn mir_graph(&self) -> &MirGraph {
        &self.mir_graph
    }

    pub fn mir_graph_mut(&mut self) -> &mut MirGraph {
        &mut self.mir_graph
    }

    pub fn get_time_build(&self) -> i64 {
        self.elapsed.0
    }

    pub fn add_time_build(&mut self, time: i64) {
        self.elapsed.0 = self.elapsed.0 + time;
    }

    pub fn mut_ref_time_build(&mut self) -> &mut i64{
        &mut self.elapsed.0
    }

    pub fn get_time_solve(&self) -> i64 {
        self.elapsed.1
    }

    pub fn add_time_solve(&mut self, time: i64) {
        self.elapsed.1 =  self.elapsed.1 + time;
    }

    pub fn mut_ref_time_solve(&mut self) -> &mut i64{
        &mut self.elapsed.1
    }
}
