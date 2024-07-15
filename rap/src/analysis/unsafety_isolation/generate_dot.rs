use std::collections::HashSet;
use std::fmt::Write;
use crate::analysis::unsafety_isolation::UnsafetyIsolationCheck;
use rustc_hir::def_id::DefId;

impl<'tcx> UnsafetyIsolationCheck<'tcx>{
    pub fn generate_dot_graphs(&self) -> Vec<String> {
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
                    for callee in self.get_callees_by_def_id(current) {
                        if !subgraph_nodes.contains(&callee) {
                            stack.push(callee.clone());
                        }
                        subgraph_nodes.insert(callee.clone());
                        // visited.insert(callee.clone());
                    }
                    for constructor in self.get_constructors_by_def_id(current) {
                        if !subgraph_nodes.contains(&constructor) {
                            stack.push(constructor.clone());
                        }
                        subgraph_nodes.insert(constructor.clone());
                        // visited.insert(constructor.clone());
                    }
                }
                let graph = self.generate_dot_desc(subgraph_nodes);
                graphs.push(graph);
            }
        }
        graphs
    }
    
    pub fn generate_dot_desc(&self,subgraph_nodes:HashSet<DefId>) -> String {
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

            // Process edges
            for &callee_id in &node.callees {
                if let Some(callee) = self.nodes.iter().find(|n| n.node_id == callee_id) {
                    edges.push((node.node_name.clone(), callee.node_name.clone(), "solid"));
                }
            }
            if let Some(constructor_ids) = node.constructor_id.clone() {
                for cons in constructor_ids {
                    if let Some(constructor) = self.nodes.iter().find(|n| n.node_id == cons) {
                        edges.push(( constructor.node_name.clone(), node.node_name.clone(), "dashed"));
                    }
                }
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

    pub fn get_node_name_by_def_id(&self, def_id: DefId) -> String{
        if let Some(node) = self.nodes.iter().find(|n| n.node_id == def_id) {
            return node.node_name.clone();
        }
        String::new()
    }

    pub fn get_callees_by_def_id(&self, def_id: DefId) -> Vec<DefId>{
        if let Some(node) = self.nodes.iter().find(|n| n.node_id == def_id) {
            return node.callees.clone();
        }
        Vec::new()
    }

    pub fn get_constructors_by_def_id(&self, def_id: DefId) -> Vec<DefId>{
        if let Some(node) = self.nodes.iter().find(|n| n.node_id == def_id) {
            if let Some(constructors) = &node.constructor_id{
                return constructors.clone();    
            }
        }
        Vec::new()
    }
}