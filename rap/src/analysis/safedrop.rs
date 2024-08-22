pub mode safedrop;

use rustc_middle::ty::TyCtxt;
use crate::{Elapsed};
use flow_analysis::{MirGraph,FlowAnalysis,IcxSliceFroBlock, IntraFlowContext};
use type_analysis::{TypeAnalysis,AdtOwner};
use std::collections::HashMap;

fn query_safedrop<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> () {
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
	        rap_info!("Over visited: {:?}", def_id); 
	    }
    }
}
