use rustc_middle::ty::TyCtxt;

use crate::analysis::core::dataflow::graph::Graph;

pub mod bounds_len;
pub mod bounds_loop_push;

pub fn check(graph: &Graph, tcx: &TyCtxt) {
    // bounds_len::check(graph, tcx);
    bounds_loop_push::check(graph, tcx);
}
