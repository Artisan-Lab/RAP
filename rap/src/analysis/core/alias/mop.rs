pub mod mop;
pub mod graph;
pub mod types;
pub mod alias;

use rustc_middle::ty::TyCtxt;
use rustc_hir::def_id::DefId;
use rustc_data_structures::fx::FxHashMap;
use crate::utils::utils::*;
use graph::MopGraph;
use alias::FnRetAlias;

pub const VISIT_LIMIT:usize = 10000;

//struct to cache the results for analyzed functions.
pub type FnMap = FxHashMap<DefId, FnRetAlias>;

pub struct MopAlias<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub fn_map: FnMap,
}

impl<'tcx> MopAlias<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self{ 
            tcx: tcx,
            fn_map: FxHashMap::default(),
        }
    }

    pub fn start(&mut self) {
        for local_def_id in self.tcx.iter_local_def_id() {
            let hir_map = self.tcx.hir();
            if hir_map.maybe_body_owned_by(local_def_id).is_some() {
                self.query_mop(local_def_id.to_def_id());
            }
        }
        for (fn_id, fn_alias) in &self.fn_map {
            let fn_name = get_fn_name(self.tcx, *fn_id);
            println!("{:?}: {:?}", fn_id, fn_name);
            println!("{:?}", fn_alias);
        }
    }

    pub fn query_mop(&mut self, def_id: DefId) -> () {
        /* filter const mir */
        if let Some(_other) = self.tcx.hir().body_const_context(def_id.expect_local()) {
            return;
        }
        if self.tcx.is_mir_available(def_id) {
            let mut mop_graph = MopGraph::new(self.tcx, def_id);
            mop_graph.solve_scc();
            mop_graph.check(0, &mut self.fn_map);
            if mop_graph.visit_times <= VISIT_LIMIT { 
                return ;
            } else { 
                println!("Over visited: {:?}", def_id); 
            }
        }
    }
}
