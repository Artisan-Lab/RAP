pub mod safedrop;
pub mod graph;
pub mod bug_records;
pub mod check_bugs;
pub mod corner_handle;
pub mod types;
pub mod alias;
pub mod utils;

use rustc_middle::ty::TyCtxt;
use rustc_hir::def_id::DefId;

use graph::SafeDropGraph;
use safedrop::*;

pub struct SafeDrop<'tcx> {
        pub tcx: TyCtxt<'tcx>,
}

impl<'tcx> SafeDrop<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self{ tcx, }
    }
    pub fn start(&self) {
        for local_def_id in self.tcx.iter_local_def_id() {
            let hir_map = self.tcx.hir();
            if hir_map.maybe_body_owned_by(local_def_id).is_some() {
                query_safedrop(self.tcx, local_def_id.to_def_id());
            }
        }
    }
}
pub fn query_safedrop<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> () {
    /* filter const mir */
    if let Some(_other) = tcx.hir().body_const_context(def_id.expect_local()) {
        return;
    }
    if tcx.is_mir_available(def_id) {
        let body = tcx.optimized_mir(def_id);
        let mut func_map = FuncMap::new();
        let mut safedrop_graph = SafeDropGraph::new(&body, tcx, def_id);
        safedrop_graph.solve_scc();
        safedrop_graph.check(0, tcx, &mut func_map);
        if safedrop_graph.visit_times <= VISIT_LIMIT { 
            safedrop_graph.report_bugs(); 
        } else { 
            println!("Over visited: {:?}", def_id); 
        }
    }
}

