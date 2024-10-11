pub mod mop;
pub mod graph;
pub mod types;
pub mod alias;

use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::DefId;
use rustc_data_structures::fx::FxHashMap;
use crate::rap_info;
//use crate::utils::source::*;
use graph::MopGraph;
use alias::FnRetAlias;

pub const VISIT_LIMIT:usize = 1000;

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

    pub fn start(&mut self) -> &FnMap {
        for local_def_id in self.tcx.iter_local_def_id() {
            let hir_map = self.tcx.hir();
            if hir_map.maybe_body_owned_by(local_def_id).is_some() {
                self.query_mop(local_def_id.to_def_id());
            }
        }
        rap_info!("Meaning of output: 0 for ret value; 1,2,3,... for corresponding args.");
        for (fn_id, fn_alias) in &self.fn_map {
            /* FIXME: This does not work.
            let fn_name = get_name(self.tcx, *fn_id);
            */
            if fn_alias.alias_vec.len() > 0 {
                rap_info!("{:?}", fn_id);
                rap_info!("{}", fn_alias);
            }
        }
        return &self.fn_map;
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
