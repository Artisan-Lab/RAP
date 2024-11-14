use once_cell::sync::OnceCell;

use rustc_middle::mir::Local;
use rustc_middle::ty::TyCtxt;

use crate::analysis::core::dataflow::graph::{
    AggKind, DFSStatus, Direction, Graph, GraphEdge, GraphNode, NodeOp,
};
use crate::analysis::utils::def_path::DefPath;
use crate::rap_warn;
use crate::utils::log::underline_span_in_the_line;

static DEFPATHS: OnceCell<DefPaths> = OnceCell::new();

struct DefPaths {
    ops_range: DefPath,
    vec_len: DefPath,
    ops_index: DefPath,
    ops_index_mut: DefPath,
}

impl DefPaths {
    pub fn new(tcx: &TyCtxt<'_>) -> Self {
        Self {
            ops_range: DefPath::new("std::ops::Range", tcx),
            vec_len: DefPath::new("std::vec::Vec::len", tcx),
            ops_index: DefPath::new("std::ops::Index::index", tcx),
            ops_index_mut: DefPath::new("std::ops::IndexMut::index_mut", tcx),
        }
    }
}

pub fn check(graph: &Graph, tcx: &TyCtxt) {
    fn extract_upperbound_node_if_ops_range(graph: &Graph, node: &GraphNode) -> Option<Local> {
        let def_paths = &DEFPATHS.get().unwrap();
        let target_def_id = def_paths.ops_range.last_def_id();
        if let NodeOp::Aggregate(AggKind::Adt(def_id)) = node.op {
            if def_id == target_def_id {
                let upperbound_edge = &graph.edges[node.in_edges[1]]; // the second field
                if let GraphEdge::NodeEdge { src, .. } = upperbound_edge {
                    return Some(*src);
                } else {
                    panic!("The upperbound edge of Agg node is not a NodeEdge");
                }
            }
        }
        None
    }

    fn find_upside_vec_len_node(graph: &Graph, node_idx: Local) -> Option<Local> {
        let mut vec_len_node_idx = None;
        let def_paths = &DEFPATHS.get().unwrap();
        let target_def_id = def_paths.vec_len.last_def_id();
        // Warning: may traverse all upside nodes and the new result will overwrite on the previous result
        let mut node_operator = |idx: Local| -> DFSStatus {
            let node = &graph.nodes[idx];
            if let NodeOp::Call(def_id) = node.op {
                if def_id == target_def_id {
                    vec_len_node_idx = Some(idx);
                    return DFSStatus::Stop;
                }
            }
            DFSStatus::Continue
        };
        graph.dfs(
            node_idx,
            Direction::Upside,
            &mut node_operator,
            &mut Graph::equivalent_edge_validator,
            false,
        );
        vec_len_node_idx
    }

    fn find_downside_index_node(graph: &Graph, node_idx: Local) -> Vec<Local> {
        let mut index_node_idxs: Vec<Local> = Vec::new();
        let def_paths = &DEFPATHS.get().unwrap();
        // Warning: traverse all downside nodes
        let mut node_operator = |idx: Local| -> DFSStatus {
            let node = &graph.nodes[idx];
            if let NodeOp::Call(def_id) = node.op {
                if def_id == def_paths.ops_index.last_def_id()
                    || def_id == def_paths.ops_index_mut.last_def_id()
                {
                    index_node_idxs.push(idx);
                }
            }
            DFSStatus::Continue
        };
        graph.dfs(
            node_idx,
            Direction::Downside,
            &mut node_operator,
            &mut Graph::always_true_edge_validator,
            true,
        );
        index_node_idxs
    }

    let _ = &DEFPATHS.get_or_init(|| DefPaths::new(tcx));
    for (node_idx, node) in graph.nodes.iter_enumerated() {
        if let Some(upperbound_node_idx) = extract_upperbound_node_if_ops_range(graph, node) {
            if let Some(vec_len_node_idx) = find_upside_vec_len_node(graph, upperbound_node_idx) {
                let maybe_vec_node_idx = graph.get_upside_idx(vec_len_node_idx, 0).unwrap();
                let maybe_vec_node_idxs = graph.collect_equivalent_locals(maybe_vec_node_idx);
                let mut index_record = Vec::new();
                for index_node_idx in find_downside_index_node(graph, node_idx).into_iter() {
                    let maybe_vec_node_idx = graph.get_upside_idx(index_node_idx, 0).unwrap();
                    if maybe_vec_node_idxs.contains(&maybe_vec_node_idx) {
                        index_record.push(index_node_idx);
                    }
                }
                report_bug(graph, upperbound_node_idx, &index_record);
            }
        }
    }
}

fn report_bug(graph: &Graph, upperbound_node_idx: Local, index_record: &Vec<Local>) {
    rap_warn!(
        "Unnecessary bounds checkings detected in function {:?}.
Index is upperbounded at:
{}
Checked at:
{}
",
        graph.def_id,
        underline_span_in_the_line(graph.nodes[upperbound_node_idx].span),
        index_record
            .iter()
            .map(|node_idx| {
                underline_span_in_the_line(graph.nodes[*node_idx].span) // buggy, what about multiple index in the same line?
            })
            .collect::<Vec<String>>()
            .join("\n")
    );
}
