use rustc_middle::ty::TyCtxt;

use crate::{RapConfig, Elapsed};
use crate::analysis::flow_analysis::MirGraph;
use crate::analysis::type_analysis::AdtOwner;

use std::collections::HashMap;

#[derive(Clone)]
pub struct RapGlobalCtxt<'tcx> {
    tcx: TyCtxt<'tcx>,
    config: RapConfig,
    adt_owner: AdtOwner,
    mir_graph: MirGraph,
    elasped: Elapsed,
}

impl<'tcx> RapGlobalCtxt<'tcx> {
    pub fn new(tcx:TyCtxt<'tcx>, config: RapConfig) -> Self {
        Self {
            tcx,
            config,
            adt_owner: HashMap::default(),
            mir_graph: HashMap::default(),
            elasped: (0, 0),
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn config(&self) -> RapConfig {
        self.config
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
        self.elasped.0
    }

    pub fn add_time_build(&mut self, time: i64) {
        self.elasped.0 = self.elasped.0 + time;
    }

    pub fn mut_ref_time_build(&mut self) -> &mut i64{
        &mut self.elasped.0
    }

    pub fn get_time_solve(&self) -> i64 {
        self.elasped.1
    }

    pub fn add_time_solve(&mut self, time: i64) {
        self.elasped.1 =  self.elasped.1 + time;
    }

    pub fn mut_ref_time_solve(&mut self) -> &mut i64{
        &mut self.elasped.1
    }
}