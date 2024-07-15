pub mod type_analysis;
pub mod flow_analysis;

use rustc_middle::ty::TyCtxt;
use crate::{Elapsed};
use crate::analysis::rcanary::flow_analysis::MirGraph;
use crate::analysis::rcanary::type_analysis::AdtOwner;
use crate::analysis::rcanary::flow_analysis::{IcxSliceFroBlock, IntraFlowContext};
use std::collections::HashMap;

#[derive(Clone)]
pub struct RcanaryGlobalCtxt<'tcx> {
    tcx: TyCtxt<'tcx>,
    adt_owner: AdtOwner,
    mir_graph: MirGraph,
    elapsed: Elapsed,
}

impl<'tcx> RcanaryGlobalCtxt<'tcx> {
    pub fn new(tcx:TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            adt_owner: HashMap::default(),
            mir_graph: HashMap::default(),
            elapsed: (0, 0),
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
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

pub trait Tcx<'tcx, 'o, 'a> {
    fn tcx(&'o self) -> TyCtxt<'tcx>;
}

pub trait Rcx<'tcx, 'o, 'a> {
    fn rcx(&'o self) -> &'a RcanaryGlobalCtxt<'tcx>;

    fn tcx(&'o self) -> TyCtxt<'tcx>;
}

pub trait RcxMut<'tcx, 'o, 'a> {
    fn rcx(&'o self) -> &'o RcanaryGlobalCtxt<'tcx>;

    fn rcx_mut(&'o mut self) -> &'o mut RcanaryGlobalCtxt<'tcx>;

    fn tcx(&'o self) -> TyCtxt<'tcx>;
}

pub trait IcxMut<'tcx, 'ctx, 'o> {
    fn icx(&'o self) -> &'o IntraFlowContext<'tcx, 'ctx>;

    fn icx_mut(&'o mut self) -> &'o mut IntraFlowContext<'tcx, 'ctx>;
}

pub trait IcxSliceMut<'tcx, 'ctx, 'o> {
    fn icx_slice(&'o self) -> &'o IcxSliceFroBlock<'tcx, 'ctx>;

    fn icx_slice_mut(&'o mut self) -> &'o mut IcxSliceFroBlock<'tcx, 'ctx>;
}
