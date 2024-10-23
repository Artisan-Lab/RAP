pub mod call_graph_helper;

use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use call_graph_helper::{Graph, Node};
use rustc_middle::ty::TyCtxt;



pub struct CallGraph<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub graph: Graph,
}

impl<'tcx> CallGraph<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx: tcx,
            graph: Graph::new(),
        }
    }

    pub fn start(&mut self) {
        for &def_id in self.tcx.mir_keys(()).iter() {
            let body = &self.tcx.optimized_mir(def_id);
            let 
        }
    }

}
