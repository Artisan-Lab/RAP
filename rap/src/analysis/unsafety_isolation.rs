pub mod isolation_graph;
pub mod hir_visitor;
pub mod generate_dot;
pub mod draw_dot;

use crate::analysis::unsafety_isolation::isolation_graph::*;
use crate::analysis::unsafety_isolation::draw_dot::render_dot_graphs;
use crate::analysis::unsafety_isolation::hir_visitor::{ContainsUnsafe, RelatedFnCollector};
use rustc_middle::{ty, ty::TyCtxt, mir::{TerminatorKind, Operand}};
use rustc_hir::def_id::DefId;
use std::collections::VecDeque;

pub struct UnsafetyIsolationCheck<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub nodes: Vec<IsolationGraphNode>,
    pub related_func_def_id: Vec<DefId>,
}

impl<'tcx> UnsafetyIsolationCheck<'tcx>{
    pub fn new(tcx: TyCtxt<'tcx>) -> Self{
        Self{
            tcx,
            nodes: Vec::new(),
            related_func_def_id: Vec::new(),
        }
    }

    pub fn start(&mut self) {
        // extract all unsafe nodes
        self.filter_and_extend_unsafe();
        // divide these nodes into several subgraphs and use dot to generate graphs
        let dot_graphs = self.generate_dot_graphs();
        render_dot_graphs(dot_graphs);
        // self.show_nodes();
    }

    pub fn filter_and_extend_unsafe(&mut self) {
        let related_items = RelatedFnCollector::collect(self.tcx);
        let hir_map = self.tcx.hir();
        let mut queue = VecDeque::new();
        let mut visited = std::collections::HashSet::new();
        
        //'related_items' is used for recording whether this api is in crate or not
        //and then init the queue, including all unsafe func and interior unsafe func
        for (_, &ref vec) in &related_items {
            for (body_id, _) in vec{
                let (function_unsafe, block_unsafe) = ContainsUnsafe::contains_unsafe(self.tcx, *body_id);
                let body_did = hir_map.body_owner_def_id(*body_id).to_def_id();
                if function_unsafe || block_unsafe {
                    let node_type = self.get_type(body_did);
                    let name = self.get_name(body_did);
                    let mut new_node = IsolationGraphNode::new(body_did, node_type, name, function_unsafe, true);
                    if node_type == 1 {
                        new_node.constructor_id = self.search_constructor(body_did);
                    }
                    self.nodes.push(new_node);
                    self.related_func_def_id.push(body_did);
                    if visited.insert(body_did) { 
                        queue.push_back(body_did);
                    }
                }
            }
        }
    
        // BFS handling the queue
        while let Some(body_did) = queue.pop_front() {
            if !self.is_crate_api_node(body_did) {
                continue;
            }
            // get all unsafe callees in current crate api and insert to queue
            let callees = self.visit_node_callees(body_did);
            for &callee_id in &callees {
                if visited.insert(callee_id) { 
                    queue.push_back(callee_id);
                }
            }
        }
    }

    pub fn check_if_node_exists(&self, body_did: DefId) -> bool {
        for node in &self.nodes {
            if node.node_id == body_did {
                return true
            }
        }
        return false
    }

    pub fn check_safety(&self, body_did: DefId) -> bool {
        let poly_fn_sig = self.tcx.fn_sig(body_did);
        let fn_sig = poly_fn_sig.skip_binder();
        fn_sig.unsafety() == rustc_hir::Unsafety::Unsafe
    }

    pub fn get_name(&self,body_did: DefId) -> String {
        let tcx = self.tcx;
        let mut name = String::new();
        if let Some(assoc_item) = tcx.opt_associated_item(body_did) {
            if let Some(impl_id) = assoc_item.impl_container(tcx) {
                // get struct name
                let ty = tcx.type_of(impl_id).skip_binder();
                let type_name = ty.to_string();
                let type_name = type_name.split('<').next().unwrap_or("").trim();
                // get method name
                let method_name = tcx.def_path(body_did).to_string_no_crate_verbose();
                let method_name = method_name.split("::").last().unwrap_or("");
                name = format!("{}.{}", type_name, method_name);
                // println!("{:?}",format!("{}.{}", type_name, method_name));
            }
            //TODO: handle trait method
        }
        else {
            let verbose_name = tcx.def_path(body_did).to_string_no_crate_verbose();
            name = verbose_name.split("::").last().unwrap_or("").to_string();
        }
        return name;
    }

    //retval: 0-constructor, 1-method, 2-function
    pub fn get_type(&self,def_id: DefId) -> usize{
        let tcx = self.tcx;
        let mut node_type = 2;
        if let Some(assoc_item) = tcx.opt_associated_item(def_id) {
            if assoc_item.fn_has_self_parameter {
                node_type = 1;
            } else if !assoc_item.fn_has_self_parameter  {
                let fn_sig = tcx.fn_sig(def_id).skip_binder();
                let output = fn_sig.output().skip_binder();
                // return type is 'Self'
                if output.is_param(0) {
                    node_type = 0;
                }
                // return type is struct's name
                if let Some(assoc_item) = tcx.opt_associated_item(def_id) {
                    if let Some(impl_id) = assoc_item.impl_container(tcx) {
                        let ty = tcx.type_of(impl_id).skip_binder();
                        if output == ty{
                            node_type = 0;
                        }
                    }
                }
            }
        }
        return node_type;
    }

    pub fn search_constructor(&mut self,def_id: DefId) -> Option<Vec<DefId>> {
        let tcx = self.tcx;
        let mut constructors = Vec::new();
        if let Some(assoc_item) = tcx.opt_associated_item(def_id) {
            if let Some(impl_id) = assoc_item.impl_container(tcx) {
                // get struct ty
                let ty = tcx.type_of(impl_id).skip_binder();
                if let Some(adt_def) = ty.ty_adt_def() {
                    let adt_def_id = adt_def.did();
                    let impl_vec = self.get_impls_for_struct(adt_def_id);
                    for impl_id in impl_vec {
                        let associated_items = tcx.associated_items(impl_id);
                        for item in associated_items.in_definition_order() {
                            if let ty::AssocKind::Fn = item.kind {
                                let item_def_id = item.def_id;
                                if self.get_type(item_def_id) == 0{
                                    constructors.push(item_def_id.clone());
                                    self.insert_node(item_def_id);
                                }
                            }
                        }
                    }
                }
            }
        }
        if constructors.is_empty() {
            None
        } else {
            Some(constructors)
        }
    }

    pub fn get_impls_for_struct(&self, struct_def_id: DefId) -> Vec<DefId> {
        let tcx = self.tcx;
        let mut impls = Vec::new();
        for item_id in tcx.hir().items() {
            let item = tcx.hir().item(item_id);
            if let rustc_hir::ItemKind::Impl(ref impl_item) = item.kind {
                if let rustc_hir::TyKind::Path(ref qpath) = impl_item.self_ty.kind {
                    if let rustc_hir::QPath::Resolved(_, ref path) = qpath {
                        if let rustc_hir::def::Res::Def(_, ref def_id) = path.res {
                            if *def_id == struct_def_id {
                                impls.push(item.owner_id.to_def_id());
                            }
                        }
                    }
                }
            }
        }
        impls
    }    

    // visit the func body, record all its unsafe callees and modify visited_tag
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
                                if self.check_safety(*callee_def_id) {
                                    if !callees.contains(callee_def_id) {
                                        callees.push(*callee_def_id);
                                        if !self.check_if_node_exists(*callee_def_id) {
                                            self.insert_node(*callee_def_id);                           
                                        }
                                    }
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
            node.visited_tag = true;
        }
        return callees
    }

    pub fn is_crate_api_node(&self, body_did: DefId) -> bool {
        if let Some(node) = self.nodes.iter().find(|n| n.node_id == body_did) {
            return node.is_crate_api
        }
        false
    }

    pub fn insert_node(&mut self,body_did: DefId){
        if self.check_if_node_exists(body_did){
            return
        }
        let node_type = self.get_type(body_did);
        let name = self.get_name(body_did);
        let is_crate_api = self.related_func_def_id.contains(&body_did);
        let mut new_node = IsolationGraphNode::new(body_did, node_type, name, true, is_crate_api);
        if node_type == 1 {
            new_node.constructor_id = self.search_constructor(body_did);
        }
        new_node.visited_tag = false;
        self.nodes.push(new_node);
    }

    pub fn show_nodes(&self) {
        for node in &self.nodes{
            println!("name:{:?},safety:{:?},calles:{:?}",node.node_name,node.node_unsafety,node.callees);
        }
    }
}
