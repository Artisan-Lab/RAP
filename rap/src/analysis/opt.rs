pub mod checking;

use rustc_middle::ty::TyCtxt;

use super::core::dataflow::DataFlow;
use checking::bounds_checking::check;

pub struct Opt<'tcx> {
    pub tcx: TyCtxt<'tcx>,
}

impl<'tcx> Opt<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self { tcx }
    }

    pub fn start(&mut self) {
        let mut dataflow = DataFlow::new(self.tcx, false);
        dataflow.build_graphs();
        for (_, graph) in dataflow.graphs.iter() {
            check(graph, &self.tcx);
        }
    }
}
