pub mod mop;
pub mod graph;
pub mod types;
pub mod alias;

use rustc_middle::ty::TyCtxt;
use rustc_hir::def_id::DefId;
use rustc_data_structures::fx::FxHashMap;
use graph::MopGraph;
use mop::*;

pub struct MopAlias<'tcx> {
        pub tcx: TyCtxt<'tcx>,
}

impl<'tcx> MopAlias<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self{ tcx, }
    }
    pub fn start(&self) {
        for local_def_id in self.tcx.iter_local_def_id() {
            let hir_map = self.tcx.hir();
            if hir_map.maybe_body_owned_by(local_def_id).is_some() {
                query_mop(self.tcx, local_def_id.to_def_id());
            }
        }
    }
}
pub fn query_mop<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> () {
    /* filter const mir */
    if let Some(_other) = tcx.hir().body_const_context(def_id.expect_local()) {
        return;
    }
    if tcx.is_mir_available(def_id) {
        let body = tcx.optimized_mir(def_id);
        let mut fn_map = FxHashMap::default();
        let mut mop_graph = MopGraph::new(&body, tcx, def_id);
        mop_graph.solve_scc();
        mop_graph.check(0, tcx, &mut fn_map);
        if mop_graph.visit_times <= VISIT_LIMIT { 
            return ;
        } else { 
            println!("Over visited: {:?}", def_id); 
        }
    }
}

