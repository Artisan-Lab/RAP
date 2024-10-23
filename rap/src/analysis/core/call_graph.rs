pub mod call_graph_helper;
pub mod call_graph_visitor;

use call_graph_helper::CallGraphInfo;
use call_graph_visitor::CallGraphVisitor;
use rustc_middle::ty::TyCtxt;



pub struct CallGraph<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub graph: CallGraphInfo,
}

impl<'tcx> CallGraph<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx: tcx,
            graph: CallGraphInfo::new(),
        }
    }

    pub fn start(&mut self) {
        for &def_id in self.tcx.mir_keys(()).iter() {
            let body = &self.tcx.optimized_mir(def_id);
            let mut call_graph_visitor = CallGraphVisitor::new(self.tcx, def_id.into(), body, &mut self.graph);
            call_graph_visitor.visit();
        }
        self.graph.print_call_graph();
    }

}
