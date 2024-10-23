use std::{collections::HashMap, hash::Hash};

use rustc_middle::mir;
use rustc_middle::ty::TyCtxt;
use rustc_hir::def_id::DefId;
use rustc_middle::ty::FnDef;
use crate::analysis::core::alias::mop::types::TyKind;


#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Node {
    def_id: DefId,
    def_path: String,
}

impl Node {
    pub fn new(def_id: DefId, def_path: &String) -> Self {
        Self {
            def_id: def_id,
            def_path: def_path.clone(),
        }
    }

    pub fn get_def_id(&self) -> DefId {
        self.def_id
    }

    pub fn get_def_path(&self) -> String {
        self.def_path.clone()
    }
}

pub struct CallGraphInfo {
    pub functions: HashMap<usize, Node>,    // id -> node
    pub function_calls: Vec<(usize, usize)>,    // (id, id)
    pub node_registry: HashMap<String, usize>,  // path -> id
}

impl CallGraphInfo {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            function_calls: Vec::new(),
            node_registry: HashMap::new(),
        }
    }

    pub fn get_node_num(&self) -> usize {
        self.functions.len()
    }

    pub fn add_node(& mut self, def_id: DefId, def_path: &String) {
        if let None = self.get_noed_by_path(def_path) {
            let id = self.node_registry.len();
            let node = Node::new(def_id, def_path);
            self.node_registry.insert(def_path.clone(), id);
            self.functions.insert(id, node);
        }
    }

    pub fn add_funciton_call_edge(& mut self, caller_id: usize, callee_id: usize) {
        self.function_calls.push((caller_id, callee_id));
    }

    pub fn get_noed_by_path(&self, def_path: &String) -> Option<usize> {
        if let Some(&id) = self.node_registry.get(def_path) {
            Some(id)
        } else  {
            None
        }
    }

    pub fn print_call_graph(&self) {
        println!("CallGraph Analysis:");
        println!("There are {} functions calls!", self.function_calls.len());
        for call in self.function_calls.clone() {
            let caller_id = call.0;
            let callee_id = call.1;
            if let Some(caller_node) = self.functions.get(&caller_id) {
                if let Some(callee_node) = self.functions.get(&callee_id) {
                    let caller_def_path = caller_node.get_def_path();
                    let callee_def_path = callee_node.get_def_path();
                    println!("{}:{} -> {}:{}", call.0, caller_def_path, call.1, callee_def_path);
                }
            }
        }
        println!("There are {} functions", self.functions.len());
        for (id, node) in self.functions.clone() {
            println!("{}:{}", id, node.get_def_path());
        }
    }
}

pub struct CallGraphVisitor<'b, 'tcx> {
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body: &'tcx mir::Body<'tcx>,
    call_graph_info: &'b mut CallGraphInfo,
}

impl<'b, 'tcx> CallGraphVisitor<'b, 'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, def_id: DefId, body: &'tcx mir::Body<'tcx>, call_graph_info: &'b mut CallGraphInfo) -> Self {
        Self {
            tcx: tcx,
            def_id: def_id,
            body: body,
            call_graph_info: call_graph_info,
        }
    }

    pub fn add_in_call_graph(&mut self, caller_def_path: &String, callee_def_id: DefId, callee_def_path: &String) {
        if let Some(caller_id) = self.call_graph_info.get_noed_by_path(caller_def_path) {
            if let Some(callee_id) = self.call_graph_info.get_noed_by_path(callee_def_path) {
                self.call_graph_info.add_funciton_call_edge(caller_id, callee_id);
            } else {
                self.call_graph_info.add_node(callee_def_id, callee_def_path);
                if let Some(callee_id) = self.call_graph_info.get_noed_by_path(callee_def_path) {
                    self.call_graph_info.add_funciton_call_edge(caller_id, callee_id);
                }
            }
        }
    }

    pub fn visit(&mut self) {
        let caller_path_str = self.tcx.def_path_str(self.def_id);
        self.call_graph_info.add_node(self.def_id, &caller_path_str);
        for (index, data) in self.body.basic_blocks.iter().enumerate() {
            let terminator = data.terminator();

        } 
    } 

    fn visit_terminator(&mut self, terminator: &mir::Terminator<'tcx>) {
        if let mir::TerminatorKind::Call {
            func,
            ..
        } = &terminator.kind {
            if let mir::Operand::Constant(constant) = func {
                if let FnDef(callee_def_id, callee_substs) = constant.const_.ty().kind() {
                    if 
                }
            }
        }
    } 


    fn my_visit_terminator(
        &mut self,
        terminator: & mir::Terminator<'tcx>,
    ){
        match &terminator.kind{
            mir::TerminatorKind::Call {
                func,
                ..  
            } => {
                match func{
                    mir::Operand::Constant(constant) => {
                        if let TyKind::FnDef(callee_def_id, callee_substs) = constant.literal.ty.kind{
                             if !is_std_crate(&self.tcx.crate_name(callee_def_id.krate).to_string()){ 
                                 let param_env = self.tcx.param_env(self.def_id);
                                 if let Ok(Some(instance)) = Instance::resolve(self.tcx, param_env, callee_def_id, callee_substs){
                                     let mut instance_def_id = None;
                                     match instance.def{
                                         InstanceDef::Item(def_id) => {
                                             instance_def_id = Some(def_id.def_id_for_type_of());
                                        // println!("instance_callee_def_path: {}", get_fn_path(&self.tcx, instance_def_id.def_id_for_type_of()));
                                         }
                                         InstanceDef::Intrinsic(def_id)
                                         | InstanceDef::CloneShim(def_id, _) => {
                                             if !self.tcx.is_closure(def_id){
                                                 instance_def_id = Some(def_id);
                                             // println!("instance_callee_def_path: {}", get_fn_path(&self.tcx, instance_def_id));
                                             } 
                                         }
                                         _ => {}
                                     }
                                     if let Some(instance_def_id) = instance_def_id{
                                         if instance_def_id == self.def_id{
                                             let caller_def_path = get_fn_path(&self.tcx, self.def_id);
                                             let callee_def_path = get_fn_path(&self.tcx, instance_def_id); 
                                             let location = get_fn_location(&self.tcx, instance_def_id);
                                             let msg = "\x1b[031mwarning!! find a recursion function which may cause stackoverflow\x1b[0m";
                                             println!("{}", instance);
                                             progress_info!("{}: {}->{}; \x1b[031mlocation\x1b[0m: {}", msg, caller_def_path, callee_def_path, location); 
                                         }
                                         let caller_def_path = get_fn_path(&self.tcx, self.def_id);
                                         let callee_def_path = get_fn_path(&self.tcx, instance_def_id);
                                        // let location = get_fn_location(&self.tcx, instance_def_id);
                                        // println!("instance_callee_def_path: {}; location: {}", callee_def_path, location);
                                         self.add_in_call_graph(&caller_def_path, instance_def_id, &callee_def_path);
                                     }
                                 }
                                 else{
                                     if self.def_id == callee_def_id{
                                         let caller_def_path = get_fn_path(&self.tcx, self.def_id);
                                         let callee_def_path = get_fn_path(&self.tcx, callee_def_id); 
                                         let location = get_fn_location(&self.tcx, callee_def_id);  
                                         let msg = "\x1b[031mwarning!! find a recursion function which may cause stackoverflow\x1b[0m";
                                         progress_info!("{}: {}->{}; \x1b[031mlocation\x1b[0m: {}", msg, caller_def_path, callee_def_path,location); 
                                     }
                                     let caller_def_path = get_fn_path(&self.tcx, self.def_id);
                                     let callee_def_path = get_fn_path(&self.tcx, callee_def_id);
                                     //let location = get_fn_location(&self.tcx, callee_def_id);
                                     //println!("callee: {}; location: {}", callee_def_path, location);
                                     self.add_in_call_graph(&caller_def_path, callee_def_id, &callee_def_path);
                                 }
                             }
                        }
                    }
                    _ => {}
                 } 
              }
            _ => {}
        }
    }


}




