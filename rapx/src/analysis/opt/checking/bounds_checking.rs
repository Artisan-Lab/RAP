use rustc_middle::ty::TyCtxt;

use crate::analysis::core::dataflow::graph::Graph;
use crate::analysis::opt::OptCheck;

pub mod bounds_len;
pub mod bounds_loop_push;

use bounds_len::BoundsLenCheck;
use bounds_loop_push::BoundsLoopPushCheck;

pub struct BoundsCheck {
    bounds_len: BoundsLenCheck,
    bounds_loop_push: BoundsLoopPushCheck,
}

impl OptCheck for BoundsCheck {
    fn new() -> Self {
        Self {
            bounds_len: BoundsLenCheck::new(),
            bounds_loop_push: BoundsLoopPushCheck::new(),
        }
    }

    fn check(&mut self, graph: &Graph, tcx: &TyCtxt) {
        self.bounds_len.check(graph, tcx);
        self.bounds_loop_push.check(graph, tcx);
    }

    fn report(&self, graph: &Graph) {
        self.bounds_len.report(graph);
        self.bounds_loop_push.report(graph);
    }
}
