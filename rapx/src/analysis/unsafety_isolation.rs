pub mod draw_dot;
pub mod generate_dot;
pub mod hir_visitor;
pub mod isolation_graph;

use crate::analysis::unsafety_isolation::draw_dot::render_dot_graphs;
use crate::analysis::unsafety_isolation::generate_dot::UigUnit;
use crate::analysis::unsafety_isolation::hir_visitor::{ContainsUnsafe, RelatedFnCollector};
use crate::analysis::unsafety_isolation::isolation_graph::*;
use rustc_hir::def_id::DefId;
use rustc_middle::{
    mir::{Operand, TerminatorKind},
    ty,
    ty::TyCtxt,
};
use std::collections::VecDeque;

#[derive(PartialEq)]
pub enum UigInstruction {
    Doc,
    Upg,
    Ucons,
    UigCount,
}

pub struct UnsafetyIsolationCheck<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub nodes: Vec<IsolationGraphNode>,
    pub related_func_def_id: Vec<DefId>,
    pub uigs: Vec<UigUnit>,
}

impl<'tcx> UnsafetyIsolationCheck<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            nodes: Vec::new(),
            related_func_def_id: Vec::new(),
            uigs: Vec::new(),
        }
    }

    pub fn start(&mut self, ins: UigInstruction) {
        if ins == UigInstruction::Upg {
            self.generate_upg();
            return;
        }
        let related_items = RelatedFnCollector::collect(self.tcx);
        let hir_map = self.tcx.hir();
        let mut ufunc = 0;
        let mut interior_ufunc = 0;
        let mut type_vec = vec![0; 10];
        for (_, &ref vec) in &related_items {
            for (body_id, span) in vec {
                let (function_unsafe, block_unsafe) =
                    ContainsUnsafe::contains_unsafe(self.tcx, *body_id);
                let def_id = hir_map.body_owner_def_id(*body_id).to_def_id();
                if function_unsafe {
                    ufunc = ufunc + 1;
                    if ins == UigInstruction::Doc {
                        self.check_doc(def_id);
                    }
                    if ins == UigInstruction::Ucons {
                        if self.get_type(def_id) == 0 {
                            println!(
                                "Find unsafe constructor: {:?}, location:{:?}.",
                                def_id, span
                            );
                        }
                    }
                }
                if block_unsafe {
                    interior_ufunc = interior_ufunc + 1;
                    if ins == UigInstruction::UigCount {
                        self.count_uig(def_id, &mut type_vec);
                    }
                }
            }
        }
        if ins == UigInstruction::UigCount {
            println!("{:?}", type_vec);
            println!("total uig number: {:?}", type_vec.iter().sum::<usize>());
            println!(
                "------unsafe api: {:?}, interior func: {:?}------",
                ufunc, interior_ufunc
            );
        }
    }

    pub fn generate_upg(&mut self) {
        // extract all unsafe nodes
        self.filter_and_extend_unsafe();
        // divide these nodes into several subgraphs and use dot to generate graphs
        let dot_graphs = self.generate_upg_dot();
        render_dot_graphs(dot_graphs);
    }

    pub fn check_doc(&self, def_id: DefId) {
        if !self.check_if_unsafety_doc_exists(def_id) {
            let visibility = self.tcx.visibility(def_id);
            println!(
                "Lack of unsafety doc: {:?}, visibility:{:?}.",
                self.tcx.def_span(def_id),
                visibility
            );
        }
    }

    pub fn count_uig(&mut self, def_id: DefId, type_vec: &mut Vec<usize>) {
        let caller_type = self.get_type(def_id);
        let caller_cons = self.get_constructor_nodes_by_def_id(def_id);
        let callees = self.visit_node_callees(def_id);
        if callees.is_empty() {
            // single node
            Self::update_type_vec(type_vec, 3, 3, false, false);
        }
        for callee in callees {
            let callee_type = self.get_type(callee);
            let callee_cons = self.get_constructor_nodes_by_def_id(callee);
            let (two_safety1, only1) = self.is_cons_have_tow_safety(&caller_cons);
            let (two_safety2, only2) = self.is_cons_have_tow_safety(&callee_cons);
            if !two_safety1 && !two_safety2 {
                Self::update_type_vec(type_vec, caller_type, callee_type, only1, only2);
            } else if two_safety1 && !two_safety2 {
                Self::update_type_vec(type_vec, caller_type, callee_type, false, only2);
                Self::update_type_vec(type_vec, caller_type, callee_type, true, only2);
            } else if !two_safety1 && two_safety2 {
                Self::update_type_vec(type_vec, caller_type, callee_type, only1, false);
                Self::update_type_vec(type_vec, caller_type, callee_type, only1, true);
            } else {
                Self::update_type_vec(type_vec, caller_type, callee_type, false, false);
                Self::update_type_vec(type_vec, caller_type, callee_type, false, true);
                Self::update_type_vec(type_vec, caller_type, callee_type, true, false);
                Self::update_type_vec(type_vec, caller_type, callee_type, true, true);
            }
        }
    }

    // (first_flag, second_flag): if this vec contains two types of constructors' safety,
    // 'first_flag' is set true; otherwise, 'second_flag' is set as the safety of constructor's safety
    fn is_cons_have_tow_safety(&self, vec: &Vec<DefId>) -> (bool, bool) {
        let mut flag = false;
        if vec.is_empty() {
            return (false, false);
        }
        let cur = self.get_node_unsafety_by_def_id(vec[0].clone());
        for cons in vec {
            let safety = self.get_node_unsafety_by_def_id(*cons);
            if safety != cur {
                flag = true;
                break;
            }
        }
        return (flag, cur);
    }

    pub fn filter_and_extend_unsafe(&mut self) {
        let related_items = RelatedFnCollector::collect(self.tcx);
        let hir_map = self.tcx.hir();
        let mut queue = VecDeque::new();
        let mut visited = std::collections::HashSet::new();

        //'related_items' is used for recording whether this api is in crate or not
        //then init the queue, including all unsafe func and interior unsafe func
        for (_, &ref vec) in &related_items {
            for (body_id, _) in vec {
                let (function_unsafe, block_unsafe) =
                    ContainsUnsafe::contains_unsafe(self.tcx, *body_id);
                let body_did = hir_map.body_owner_def_id(*body_id).to_def_id();
                if function_unsafe || block_unsafe {
                    let node_type = self.get_type(body_did);
                    let name = self.get_name(body_did);
                    let mut new_node =
                        IsolationGraphNode::new(body_did, node_type, name, function_unsafe, true);
                    if node_type == 1 {
                        new_node.constructors = self.search_constructor(body_did);
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

    fn check_if_unsafety_doc_exists(&self, def_id: DefId) -> bool {
        if def_id.krate == rustc_hir::def_id::LOCAL_CRATE {
            let attrs = self.tcx.get_attrs_unchecked(def_id);
            for attr in attrs {
                if attr.is_doc_comment() {
                    return true;
                }
            }
        }
        return false;
    }

    pub fn check_if_node_exists(&self, body_did: DefId) -> bool {
        if let Some(_node) = self.nodes.iter().find(|n| n.node_id == body_did) {
            return true;
        }
        return false;
    }

    pub fn check_safety(&self, body_did: DefId) -> bool {
        let poly_fn_sig = self.tcx.fn_sig(body_did);
        let fn_sig = poly_fn_sig.skip_binder();
        fn_sig.safety() == rustc_hir::Safety::Unsafe
    }

    pub fn get_name(&self, body_did: DefId) -> String {
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
            }
        } else {
            let verbose_name = tcx.def_path(body_did).to_string_no_crate_verbose();
            name = verbose_name.split("::").last().unwrap_or("").to_string();
        }
        return name;
    }

    //retval: 0-constructor, 1-method, 2-function
    pub fn get_type(&self, def_id: DefId) -> usize {
        let tcx = self.tcx;
        let mut node_type = 2;
        if let Some(assoc_item) = tcx.opt_associated_item(def_id) {
            if assoc_item.fn_has_self_parameter {
                node_type = 1;
            } else if !assoc_item.fn_has_self_parameter {
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
                        if output == ty {
                            node_type = 0;
                        }
                    }
                }
            }
        }
        return node_type;
    }

    pub fn search_constructor(&mut self, def_id: DefId) -> Vec<DefId> {
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
                                if self.get_type(item_def_id) == 0 {
                                    constructors.push(item_def_id);
                                    self.check_and_insert_node(item_def_id);
                                    self.set_method_for_constructor(item_def_id, def_id);
                                }
                            }
                        }
                    }
                }
            }
        }
        constructors
    }

    pub fn get_cons_counts(&self, def_id: DefId) -> Vec<DefId> {
        let tcx = self.tcx;
        let mut constructors = Vec::new();
        let mut methods = Vec::new();
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
                                if self.get_type(item_def_id) == 0 {
                                    constructors.push(item_def_id);
                                } else if self.get_type(item_def_id) == 1 {
                                    methods.push(item_def_id);
                                }
                            }
                        }
                    }
                }
                print!("struct:{:?}", ty);
            }
        }
        println!("--------methods:{:?}", methods.len());
        constructors
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

    // visit the func body and record all its unsafe callees and modify visited_tag
    pub fn visit_node_callees(&mut self, def_id: DefId) -> Vec<DefId> {
        let mut callees = Vec::new();
        let tcx = self.tcx;
        if tcx.is_mir_available(def_id) {
            let body = tcx.optimized_mir(def_id);
            for bb in body.basic_blocks.iter() {
                match &bb.terminator().kind {
                    TerminatorKind::Call { func, .. } => {
                        if let Operand::Constant(func_constant) = func {
                            if let ty::FnDef(ref callee_def_id, _) =
                                func_constant.const_.ty().kind()
                            {
                                if self.check_safety(*callee_def_id) {
                                    if !callees.contains(callee_def_id) {
                                        callees.push(*callee_def_id);
                                        if !self.check_if_node_exists(*callee_def_id) {
                                            self.check_and_insert_node(*callee_def_id);
                                            self.set_caller_for_callee(def_id, *callee_def_id);
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
        return callees;
    }

    pub fn is_crate_api_node(&self, body_did: DefId) -> bool {
        return self.related_func_def_id.contains(&body_did);
    }

    pub fn check_and_insert_node(&mut self, body_did: DefId) {
        if self.check_if_node_exists(body_did) {
            return;
        }
        let node_type = self.get_type(body_did);
        let name = self.get_name(body_did);
        let is_crate_api = self.is_crate_api_node(body_did);
        let node_safety = self.check_safety(body_did);
        let mut new_node =
            IsolationGraphNode::new(body_did, node_type, name, node_safety, is_crate_api);
        if node_type == 1 {
            new_node.constructors = self.search_constructor(body_did);
        }
        new_node.visited_tag = false;
        self.nodes.push(new_node);
    }

    pub fn set_method_for_constructor(&mut self, constructor_did: DefId, method_did: DefId) {
        if let Some(node) = self
            .nodes
            .iter_mut()
            .find(|node| node.node_id == constructor_did)
        {
            if !node.methods.contains(&method_did) {
                node.methods.push(method_did);
            }
        }
    }

    pub fn set_caller_for_callee(&mut self, caller_did: DefId, callee_did: DefId) {
        if let Some(node) = self
            .nodes
            .iter_mut()
            .find(|node| node.node_id == callee_did)
        {
            if !node.callers.contains(&caller_did) {
                node.callers.push(caller_did);
            }
        }
    }

    pub fn show_nodes(&self) {
        for node in &self.nodes {
            println!(
                "name:{:?},safety:{:?},calles:{:?}",
                node.node_name, node.node_unsafety, node.callees
            );
        }
    }
}
