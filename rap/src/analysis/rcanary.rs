pub mod ranalyzer;

use rustc_middle::ty::TyCtxt;

use std::collections::HashMap;
use crate::Elapsed;

use ranalyzer::{MirGraph,FlowAnalysis,IcxSliceFroBlock, IntraFlowContext};
use crate::analysis::core::heap_item::{TypeAnalysis,AdtOwner};

#[allow(non_camel_case_types)]
#[derive(Clone)]
pub struct rCanary<'tcx> {
    tcx: TyCtxt<'tcx>,
    adt_owner: AdtOwner,
    mir_graph: MirGraph,
    elapsed: Elapsed,
}

impl<'tcx> rCanary<'tcx> {
    pub fn new(tcx:TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            adt_owner: HashMap::default(),
            mir_graph: HashMap::default(),
            elapsed: (0, 0),
        }
    }

    pub fn start(&mut self) {
        let rcx_boxed = Box::new(rCanary::new(self.tcx));
        let rcx = Box::leak(rcx_boxed);
        TypeAnalysis::new(rcx).start();
        FlowAnalysis::new(rcx).start();
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
    fn rcx(&'o self) -> &'a rCanary<'tcx>;

    fn tcx(&'o self) -> TyCtxt<'tcx>;
}

pub trait RcxMut<'tcx, 'o, 'a> {
    fn rcx(&'o self) -> &'o rCanary<'tcx>;

    fn rcx_mut(&'o mut self) -> &'o mut rCanary<'tcx>;

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
