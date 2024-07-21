use rustc_middle::{mir::{TerminatorKind, Operand}};
use rustc_middle::ty::{self,TyCtxt};
use rustc_hir::{def_id::DefId,intravisit::Visitor,BodyId,HirId,ItemKind};
use rustc_span::Span;
use rustc_data_structures::fx::FxHashMap;
use std::collections::HashSet;
use crate::{rap_info,rap_debug};

/* 
   The graph simply records all pairs of callers and callees; 
   TODO: it can be extended, e.g.,
     1) to manage the graph as a linked list of function nodes
     2) to record all attributes of each function
*/
pub struct CallGraph<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub edges: HashSet<(DefId, DefId)>,
}

impl<'tcx> CallGraph<'tcx>{
    pub fn new(tcx: TyCtxt<'tcx>) -> Self{
        Self{
            tcx,
            edges: HashSet::new(),
        }
    }

    pub fn start(&mut self) {
	rap_info!("Start callgraph analysis");
        let fn_items = FnCollector::collect(self.tcx);
 	rap_debug!("{:?}", fn_items);
	for (_, &ref vec) in & fn_items {
            for (body_id, _) in vec{
		let body_did = self.tcx.hir().body_owner_def_id(*body_id).to_def_id();
 		self.find_callees(body_did);
	    }
	}
	rap_info!("Show all edges of the call graph:");
        for (caller, callee) in &self.edges {
            rap_info!("  {} -> {}", self.tcx.def_path_str(*caller), self.tcx.def_path_str(*callee));
        }
    }

    pub fn find_callees(&mut self,def_id: DefId) {
        let tcx = self.tcx;
        if tcx.is_mir_available(def_id) {
            let body = tcx.optimized_mir(def_id);
            for bb in body.basic_blocks.iter() {
                match &bb.terminator().kind {
                    TerminatorKind::Call{func, ..} => {
                        if let Operand::Constant(func_constant) = func{
                            if let ty::FnDef(ref callee_def_id, _) = func_constant.const_.ty().kind() {
				self.edges.insert((def_id,*callee_def_id));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}


/// Maps `HirId` of a type to `BodyId` of related impls.
pub type FnMap = FxHashMap<Option<HirId>, Vec<(BodyId, Span)>>;

pub struct FnCollector {
    fn_map: FnMap,
}

impl FnCollector {
    pub fn collect<'tcx>(tcx: TyCtxt<'tcx>) -> FnMap {
        let mut collector = FnCollector {
            fn_map: FnMap::default(),
        };
        tcx.hir().visit_all_item_likes_in_crate(&mut collector);
        collector.fn_map
    }
}

impl<'tcx> Visitor<'tcx> for FnCollector {
    fn visit_item(&mut self, item: &'tcx rustc_hir::Item<'tcx>) {
        match &item.kind {
            ItemKind::Fn(_fn_sig, _generics, body_id) => {
                let key = Some(body_id.hir_id);
                let entry = self.fn_map.entry(key).or_insert(Vec::new());
                entry.push((*body_id, item.span));
            }
            _ => (),
        }
    }
}
