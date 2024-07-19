use rustc_middle::{mir::{TerminatorKind, Operand}};
use rustc_middle::ty::{self,TyCtxt};
use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{def_id::DefId,intravisit::Visitor,Block, BodyId, Body, HirId, Impl, ItemKind};
use rustc_span::Span;
use crate::{rap_info,rap_debug};

#[derive(Debug, Clone)]
pub struct FnNode {
    pub node_id: DefId,
    pub node_type: usize, //0:constructor, 1:method, 2:function
    pub node_name: String,
    pub node_unsafety: bool,
    pub constructors : Vec<DefId>,//if this node is a method, then it may have constructors
    pub callees: Vec<DefId>,//record all unsafe callees
    pub methods: Vec<DefId>,//tag if this node has been visited for its unsafe callees
    pub callers: Vec<DefId>,
    pub is_visited: bool,
    pub is_crate_api: bool,//record the source of the func
}

impl FnNode{
    pub fn new(node_id:DefId, node_type:usize, node_name: String, node_unsafety: bool, is_crate_api: bool) -> Self{
        Self {
            node_id,
            node_type,
            node_name,
            node_unsafety,
            constructors: Vec::new(),
            callees: Vec::new(),
            methods: Vec::new(),
            callers: Vec::new(),
            is_visited: false,
            is_crate_api,
        }
    }
}

pub struct CallGraph<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub nodes: Vec<FnNode>,
    pub related_func_def_id: Vec<DefId>,
}

impl<'tcx> CallGraph<'tcx>{
    pub fn new(tcx: TyCtxt<'tcx>) -> Self{
        Self{
            tcx,
            nodes: Vec::new(),
            related_func_def_id: Vec::new(),
        }
    }

    pub fn start(&mut self) {
	rap_info!("start callgraph analysis");
        let fn_items = FnCollector::collect(self.tcx);
	rap_debug!("{:?}", fn_items);
    }

    pub fn visit_node_callees(&mut self,def_id: DefId) -> Vec<DefId> {
        let mut callees = Vec::new();
        let tcx = self.tcx;
        if tcx.is_mir_available(def_id) {
            let body = tcx.optimized_mir(def_id);
            for bb in body.basic_blocks.iter() {
                match &bb.terminator().kind {
                    TerminatorKind::Call{func, ..} => {
                        if let Operand::Constant(func_constant) = func{
                            if let ty::FnDef(ref callee_def_id, _) = func_constant.const_.ty().kind() {
                                if !callees.contains(callee_def_id) {
                                    callees.push(*callee_def_id);
                                    //if !self.check_if_node_exists(*callee_def_id) {
                                      //  self.check_and_insert_node(*callee_def_id);  
                                      //  self.set_caller_for_callee(def_id,*callee_def_id);                        
                                    //}
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(node) = self.nodes.iter_mut().find(|n| n.node_id == def_id) {
            node.callees = callees.clone();
            node.is_visited = true;
        }
        return callees
    }
}


/// Maps `HirId` of a type to `BodyId` of related impls.
pub type RelatedItemMap = FxHashMap<Option<HirId>, Vec<(BodyId, Span)>>;

pub struct FnCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    hash_map: RelatedItemMap,
}

impl<'tcx> FnCollector<'tcx> {
    pub fn collect(tcx: TyCtxt<'tcx>) -> RelatedItemMap {
        let mut collector = FnCollector {
            tcx,
            hash_map: RelatedItemMap::default(),
        };

        tcx.hir().visit_all_item_likes_in_crate(&mut collector);

        collector.hash_map
    }
}

impl<'tcx> Visitor<'tcx> for FnCollector<'tcx> {
    fn visit_item(&mut self, item: &'tcx rustc_hir::Item<'tcx>) {
        let hir_map = self.tcx.hir();
        match &item.kind {
            ItemKind::Impl(Impl {
                unsafety: _unsafety,
                generics: _generics,
                self_ty,
                items: impl_items,
                ..
            }) => {
                let key = Some(self_ty.hir_id);
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.extend(impl_items.iter().filter_map(|impl_item_ref| {
                    let hir_id = impl_item_ref.id.hir_id();
                    hir_map
                        .maybe_body_owned_by(hir_id.owner.def_id)
                        .map(|body_id| (body_id, impl_item_ref.span))
                }));
            }
            ItemKind::Fn(_fn_sig, _generics, body_id) => {
                let key = Some(body_id.hir_id);
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.push((*body_id, item.span));
            }
            _ => (),
        }
    }
}
