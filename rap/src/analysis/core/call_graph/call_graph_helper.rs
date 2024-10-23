use std::{collections::HashMap, hash::Hash};
use rustc_hir::def_id::DefId;


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



