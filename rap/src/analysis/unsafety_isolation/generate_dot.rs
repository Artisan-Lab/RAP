use std::collections::HashSet;
use std::fmt::Write;
use crate::analysis::unsafety_isolation::UnsafetyIsolationCheck;
use rustc_hir::def_id::DefId;

#[derive(Debug, Clone)]
pub struct UigUnit {
    pub caller: DefId,
    pub callee: DefId,
    pub caller_cons: Vec<DefId>,
    pub callee_cons: Vec<DefId>,
}

#[derive(PartialEq)]
pub enum UigOp {
    DrawPic,
    TypeCount,
}

impl<'tcx> UnsafetyIsolationCheck<'tcx>{
    pub fn generate_uig(&mut self, op:UigOp) -> Vec<String> {
        let nodes = self.nodes.clone();
        let mut graphs = Vec::new();

        for node in &nodes {
            if node.callees.len() <= 0{
                println!("{:?}",node.node_id);
            }
            for callee in &node.callees{
                let mut subgraph_nodes = HashSet::new();
                subgraph_nodes.insert(node.node_id);
                subgraph_nodes.insert(*callee);
                let mut caller_cons = Vec::new();
                let mut callee_cons = Vec::new();
                for caller_cons_id in self.get_constructor_nodes_by_def_id(node.node_id) {
                    subgraph_nodes.insert(caller_cons_id);
                    caller_cons.push(caller_cons_id);
                }
                for callee_cons_id in self.get_constructor_nodes_by_def_id(*callee) {
                    subgraph_nodes.insert(callee_cons_id);
                    callee_cons.push(callee_cons_id);
                }
                let uig_unit = UigUnit {
                    caller: node.node_id.clone(),
                    callee: *callee,
                    caller_cons,
                    callee_cons,
                };
                if op == UigOp::DrawPic{
                    let graph = self.generate_dot_desc(subgraph_nodes,false,Some(uig_unit.clone()));
                    graphs.push(graph);
                } else {
                    println!("{:?}",uig_unit);
                    self.uigs.push(uig_unit.clone());
                }
            }
        }
        if op == UigOp::TypeCount {
            self.uig_type_count();
        }
        graphs
    }

    pub fn generate_upg_dot(&self) -> Vec<String> {
        let nodes = self.nodes.clone();
        let mut graphs = Vec::new();
        let mut visited = HashSet::new();

        // create dot for each node
        for node in &nodes {
            if !visited.contains(&node.node_id) {
                let mut stack = vec![node.node_id];
                let mut subgraph_nodes = HashSet::new();
                // BFS to collect all connected nodes
                while let Some(current) = stack.pop() {
                    if !visited.insert(current) {
                        continue;
                    }
                    subgraph_nodes.insert(current);
                    for adjacent_node in self.get_adjacent_nodes_by_def_id(current) {
                        if !subgraph_nodes.contains(&adjacent_node) {
                            stack.push(adjacent_node.clone());
                        }
                    }
                }
                let graph = self.generate_dot_desc(subgraph_nodes,true,None);
                graphs.push(graph);
            }
        }
        graphs
    }
    
    pub fn generate_dot_desc(&self,subgraph_nodes:HashSet<DefId>, upg_flag: bool,uig_unit_op:Option<UigUnit>) -> String {
        let mut dot = String::new();
        writeln!(dot, "digraph APIs {{").unwrap();
        writeln!(dot, "    rankdir=LR;").unwrap();
        writeln!(dot, "    rank=same;").unwrap();
        writeln!(dot, "    fontname=\"Arial\";").unwrap();
        writeln!(dot, "    fontsize=\"12\";").unwrap();
        writeln!(dot, "    fontcolor=\"blue\";").unwrap();

        // Node definitions for cluster_above and cluster_below
        let mut above_nodes = vec![];
        let mut below_nodes = vec![];
        let mut edges = vec![];

        for &node_id in &subgraph_nodes {
            // Process nodes
            let node = self.nodes.iter().find(|n| n.node_id == node_id).unwrap();
            let color = if node.node_unsafety { "red" } else { "black" };
            let shape = match node.node_type {
                0 => "doublecircle",  // constructor
                1 => "ellipse",       // method
                2 => "box",           // function
                _ => "ellipse",       // default to method if unknown
            };
            let node_tuple = (node.node_name.clone(), shape.to_string(), color.to_string());
            if node.is_crate_api {
                above_nodes.push(node_tuple);
            } else {
                below_nodes.push(node_tuple);
            }

            if upg_flag {
                // Process UPG edges
                for &callee_id in &node.callees {
                    if let Some(callee) = self.nodes.iter().find(|n| n.node_id == callee_id) {
                        edges.push((node.node_name.clone(), callee.node_name.clone(), "solid"));
                    }
                }
                for &cons in &node.constructors {
                    if let Some(constructor) = self.nodes.iter().find(|n| n.node_id == cons) {
                        edges.push(( constructor.node_name.clone(), node.node_name.clone(), "dashed"));
                    }
                }
            } 
        }
        if !upg_flag {
            // process UIG edges
            let uig_unit = uig_unit_op.clone().unwrap();
            let caller_name = self.get_node_name_by_def_id(uig_unit.caller);
            let callee_name = self.get_node_name_by_def_id(uig_unit.callee);
            edges.push((caller_name.clone(), callee_name.clone(), "solid"));
            for caller_cons_id in uig_unit.caller_cons {
                let caller_cons_name = self.get_node_name_by_def_id(caller_cons_id);
                edges.push((caller_cons_name, caller_name.clone(), "dashed"));
            }
            for callee_cons_id in uig_unit.callee_cons {
                let callee_cons_name = self.get_node_name_by_def_id(callee_cons_id);
                edges.push((callee_cons_name, callee_name.clone(), "dashed"));
            }
        }
        
        // Write crate nodes dot description
        writeln!(dot, "    subgraph cluster_above {{").unwrap();
        for (name, shape, color) in above_nodes {
            writeln!(dot, "        \"{}\" [shape={}, style=filled, color={}, fillcolor=white];", name, shape, color).unwrap();
        }
        writeln!(dot, "    }}").unwrap();

        // Write extern nodes dot description
        writeln!(dot, "    subgraph cluster_below {{").unwrap();
        for (name, shape, color) in below_nodes {
            writeln!(dot, "        \"{}\" [shape={}, style=filled, color={}, fillcolor=white];", name, shape, color).unwrap();
        }
        writeln!(dot, "    }}").unwrap();

        // Write edges
        for (src, dst, style) in edges {
            writeln!(dot, "    \"{}\" -> \"{}\" [style={}];", src, dst, style).unwrap();
        }
        writeln!(dot, "}}").unwrap();
        dot
    }

    // type_vec[ 0:sf-uf, 1:sf-um, 2:sm-uf, 3:sm-um, 4:sm(uc)-uf, 5:sf-um(uc), 6:sm-um(uc), 7:sm(uc)-um, 8:sm(uc)-um(uc) ]
    pub fn uig_type_count(&self) {
        let mut type_vec = vec![0;10];
        for uig in &self.uigs {
            let caller_type = self.get_node_type_by_def_id(uig.caller.clone());
            let callee_type = self.get_node_type_by_def_id(uig.callee.clone());
            let mut caller_cons_unsafety = false;
            let mut callee_cons_unsafety = false;
            if uig.caller_cons.is_empty() {
                if uig.callee_cons.is_empty() {  // caller\callee cons empty [0,0]
                    Self::update_type_vec(&mut type_vec, caller_type, callee_type, caller_cons_unsafety, callee_cons_unsafety);
                } else {                        // caller\callee cons empty [0,1]
                    for callee_cons_id in &uig.callee_cons {
                        callee_cons_unsafety = self.get_node_unsafety_by_def_id(callee_cons_id.clone());
                        Self::update_type_vec(&mut type_vec, caller_type, callee_type, caller_cons_unsafety, callee_cons_unsafety);
                    }
                }
            } else {
                for caller_cons_id in &uig.caller_cons {
                    caller_cons_unsafety = self.get_node_unsafety_by_def_id(caller_cons_id.clone());
                    if uig.callee_cons.is_empty() {   // caller\callee cons empty [1,0]
                        Self::update_type_vec(&mut type_vec, caller_type, callee_type, caller_cons_unsafety, callee_cons_unsafety);
                    } else {                          // caller\callee cons empty [1,1]
                        for callee_cons_id in &uig.callee_cons {
                            callee_cons_unsafety = self.get_node_unsafety_by_def_id(callee_cons_id.clone());
                            Self::update_type_vec(&mut type_vec, caller_type, callee_type, caller_cons_unsafety, callee_cons_unsafety);
                        }
                    }
                }
            }
        }
        println!("{:?}",type_vec);
    }

    pub fn update_type_vec(vec:&mut Vec<usize>, caller_type:usize, callee_type:usize, caller_cons_unsafety:bool, callee_cons_unsafety:bool) {
        let index = match (caller_type,callee_type,caller_cons_unsafety,callee_cons_unsafety) {
            (0,0,_,_) => 0,
            (0,2,_,_) => 0,
            (2,0,_,_) => 0,
            (2,2,_,_) => 0,
            (2,1,_,false) => 1,
            (0,1,_,false) => 1,
            (1,0|2,false,_) => 2,
            (1,1,false,false) => 3,
            (1,0|2,true,_) => 4,
            (0|2,1,_,true) => 5,
            (1,1,false,true) => 6,
            (1,1,true,false) => 7,
            (1,1,true,true) => 8,
            _ => 9,
            // _ => panic!("Invalid combination"),
        };
        vec[index] = vec[index] + 1;
    }

    pub fn get_node_name_by_def_id(&self, def_id: DefId) -> String{
        if let Some(node) = self.nodes.iter().find(|n| n.node_id == def_id) {
            return node.node_name.clone();
        }
        String::new()
    }

    pub fn get_node_type_by_def_id(&self, def_id: DefId) -> usize{
        if let Some(node) = self.nodes.iter().find(|n| n.node_id == def_id) {
            return node.node_type;
        }
        2
    }

    pub fn get_node_unsafety_by_def_id(&self, def_id: DefId) -> bool{
        if let Some(node) = self.nodes.iter().find(|n| n.node_id == def_id) {
            return node.node_unsafety;
        }
        false
    }

    pub fn get_adjacent_nodes_by_def_id(&self, def_id: DefId) -> Vec<DefId>{
        let mut nodes = Vec::new();
        if let Some(node) = self.nodes.iter().find(|n| n.node_id == def_id) {
            nodes.extend(node.callees.clone());
            nodes.extend(node.methods.clone());
            nodes.extend(node.callers.clone());
            nodes.extend(node.constructors.clone());
        }
        nodes
    }

    pub fn get_constructor_nodes_by_def_id(&self, def_id: DefId) -> Vec<DefId>{
        let mut nodes = Vec::new();
        if let Some(node) = self.nodes.iter().find(|n| n.node_id == def_id) {
            nodes.extend(node.constructors.clone());
        }
        nodes
    }
}