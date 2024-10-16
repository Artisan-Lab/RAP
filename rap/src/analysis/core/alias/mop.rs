pub mod alias;
pub mod graph;
pub mod mop;
pub mod types;

use crate::analysis::core::alias::FnMap;
use crate::rap_debug;
use crate::utils::source::*;
use graph::MopGraph;
use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::fx::FxHashSet;
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::DefId;

pub const VISIT_LIMIT: usize = 100;

pub struct MopAlias<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub fn_map: FnMap,
}

impl<'tcx> MopAlias<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
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
        // Meaning of output: 0 for ret value; 1,2,3,... for corresponding args.
        for (fn_id, fn_alias) in &self.fn_map {
            let fn_name = get_fn_name(self.tcx, *fn_id);
            if fn_alias.alias_vec.len() > 0 {
                rap_debug!("{:?},{:?}", fn_name, fn_id);
                rap_debug!("{}", fn_alias);
            }
        }
        return &self.fn_map;
    }

    pub fn query_mop(&mut self, def_id: DefId) -> () {
        let fn_name = get_fn_name(self.tcx, def_id);
        rap_debug!("query_mop: {:?}", fn_name);
        /* filter const mir */
        if let Some(_other) = self.tcx.hir().body_const_context(def_id.expect_local()) {
            return;
        }

        if self.tcx.is_mir_available(def_id) {
            let mut mop_graph = MopGraph::new(self.tcx, def_id);
            mop_graph.solve_scc();
            let mut recursion_set = FxHashSet::default();
            mop_graph.check(0, &mut self.fn_map, &mut recursion_set);
            if mop_graph.visit_times <= VISIT_LIMIT {
                return;
            } else {
                println!("Over visited: {:?}", def_id);
            }
        }
    }
}
