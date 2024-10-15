use std::fmt::Write;

use rustc_middle::mir::Local;
use rustc_middle::ty::TyCtxt;

use super::graph::{AggKind, Graph, GraphEdge, GraphNode, NodeOp};

fn escaped_string(s: String) -> String {
    s.replace("{", "\\{")
        .replace("}", "\\}")
        .replace("<", "\\<")
        .replace(">", "\\>")
        .replace("\"", "\\\"")
}

impl GraphEdge {
    pub fn to_dot_graph<'tcx>(&self) -> String {
        let mut attr = String::new();
        let mut dot = String::new();
        match self {
            //label=xxx
            GraphEdge::NodeEdge { src: _, dst: _, op } => {
                write!(attr, "label=\"{}\" ", escaped_string(format!("{:?}", op))).unwrap();
            }
            GraphEdge::ConstEdge { src: _, dst: _, op } => {
                write!(attr, "label=\"{}\" ", escaped_string(format!("{:?}", op))).unwrap();
            }
        }
        match self {
            GraphEdge::NodeEdge { src, dst, op: _ } => {
                write!(dot, "{:?} -> {:?} [{}]", src, dst, attr).unwrap();
            }
            GraphEdge::ConstEdge { src, dst, op: _ } => {
                write!(dot, "{:?} -> {:?} [{}]", src, dst, attr).unwrap();
            }
        }
        dot
    }
}

impl GraphNode {
    pub fn to_dot_graph<'tcx>(
        &self,
        tcx: &TyCtxt<'tcx>,
        local: Local,
        color: Option<String>,
        is_marker: bool,
    ) -> String {
        let mut attr = String::new();
        let mut dot = String::new();
        match self.op {
            //label=xxx
            NodeOp::Nop => {
                if is_marker {
                    write!(attr, "label=\"\" style=dashed ").unwrap();
                } else {
                    write!(attr, "label=\"<f0> {:?}\" ", local).unwrap();
                }
            }
            NodeOp::Call(def_id) => {
                let func_name = tcx.def_path_str(def_id);
                if is_marker {
                    write!(
                        attr,
                        "label=\"fn {}\" style=dashed ",
                        escaped_string(func_name)
                    )
                    .unwrap();
                } else {
                    write!(
                        attr,
                        "label=\"<f0> {:?} | <f1> fn {}\" ",
                        local,
                        escaped_string(func_name)
                    )
                    .unwrap();
                }
            }
            NodeOp::Aggregate(agg_kind) => match agg_kind {
                AggKind::Adt(def_id) => {
                    let agg_name = format!("{}::{{..}}", tcx.def_path_str(def_id));
                    if is_marker {
                        write!(
                            attr,
                            "label=\"Agg {}\" style=dashed ",
                            escaped_string(agg_name)
                        )
                        .unwrap();
                    } else {
                        write!(
                            attr,
                            "label=\"<f0> {:?} | <f1> Agg {}\" ",
                            local,
                            escaped_string(agg_name)
                        )
                        .unwrap();
                    }
                }
                _ => {
                    if is_marker {
                        write!(attr, "label=\"{:?}\" style=dashed ", agg_kind).unwrap();
                    } else {
                        write!(attr, "label=\"<f0> {:?} | {:?}\" ", local, agg_kind).unwrap();
                    }
                }
            },
            _ => {
                if is_marker {
                    write!(attr, "label=\"<f1> {:?}\" style=dashed ", self.op).unwrap();
                } else {
                    write!(attr, "label=\"<f0> {:?} | <f1> {:?}\" ", local, self.op).unwrap();
                }
            }
        };
        match color {
            //color=xxx
            None => {}
            Some(color) => {
                write!(attr, "color={} ", color).unwrap();
            }
        }
        write!(dot, "{:?} [{}]", local, attr).unwrap();
        dot
    }
}

impl Graph {
    pub fn to_dot_graph<'tcx>(&self, tcx: &TyCtxt<'tcx>) -> String {
        let mut dot = String::new();
        let name = tcx.def_path_str(self.def_id);

        writeln!(dot, "digraph \"{}\" {{", &name).unwrap();
        writeln!(dot, "    node [shape=record];").unwrap();
        for (local, node) in self.nodes.iter_enumerated() {
            let node_dot = if local <= Local::from_usize(self.argc) {
                node.to_dot_graph(tcx, local, Some(String::from("red")), false)
            } else if local <= Local::from_usize(self.n_locals) {
                node.to_dot_graph(tcx, local, None, false)
            } else {
                node.to_dot_graph(tcx, local, None, true)
            };
            writeln!(dot, "    {}", node_dot).unwrap();
        }
        //edges
        for edge in self.edges.iter() {
            let edge_dot = edge.to_dot_graph();
            writeln!(dot, "    {}", edge_dot).unwrap();
        }
        writeln!(dot, "}}").unwrap();
        dot
    }
}
