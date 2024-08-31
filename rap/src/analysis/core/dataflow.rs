use rustc_index::IndexVec;
use rustc_middle::mir::{Body, Operand, Rvalue, Local, Const};
use rustc_middle::mir::StatementKind;
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::TyKind;
use rustc_middle::ty::TyCtxt;
use rustc_hir::def_id::DefId;

#[derive(Clone, Debug)]
pub enum NodeOp { //warning: the fields are related to the version of the backend rustc version
    Nop,
    Err,
    //Rvalue
    Use,
    Repeat,
    Ref,
    ThreadLocalRef,
    AddressOf,
    Len,
    Cast,
    BinaryOp,
    CheckedBinaryOp, //deprecated in the latest(1.81) nightly rustc
    NullaryOp,
    UnaryOp,
    Discriminant,
    Aggregate,
    ShallowInitBox,
    CopyForDeref,
    //TerminatorKind
    Call(DefId)
}

#[derive(Clone, Debug)]
pub enum EdgeOp {
    //Operand
    Move,
    Copy,
    Constant,
}

#[derive(Clone)]
pub struct GraphEdge {
    src: Local,
    dst: Local,
    op: EdgeOp,
}

#[derive(Clone)]
pub struct GraphNode {
    op: NodeOp,
    out_edges: Vec<EdgeIdx>,
    in_edges: Vec<EdgeIdx>,
}

impl GraphNode {
    pub fn new() -> Self {
        Self { op: NodeOp::Nop, out_edges: vec![], in_edges: vec![] }
    }
}

pub type EdgeIdx = usize;
pub type GraphNodes = IndexVec<Local, GraphNode>;
pub type GraphEdges = IndexVec<EdgeIdx, GraphEdge>;
pub struct Graph {
    pub def_id: DefId,
    pub argc: usize,
    pub nodes: GraphNodes,
    pub edges: GraphEdges,
}

impl Graph {
    fn new(def_id: DefId, argc: usize, n: usize) -> Self {
        Self { def_id, argc, nodes: GraphNodes::from_elem_n(GraphNode::new(), n), edges: GraphEdges::new() }
    }

    fn add_edge(&mut self, src: Local, dst: Local, op: EdgeOp) -> EdgeIdx {
        let edge_idx = self.edges.push(GraphEdge {src, dst, op});
        self.nodes[dst].in_edges.push(edge_idx);
        self.nodes[src].out_edges.push(edge_idx);
        edge_idx
    }

    fn add_operand(&mut self, operand: &Operand, dst: Local) {
        match operand {
            Operand::Copy(place) => {
                self.add_edge(place.local, dst, EdgeOp::Copy);
            },
            Operand::Move(place) => {
                self.add_edge(place.local, dst, EdgeOp::Move);
            },
            _ => (), // Const
        }
    }

    fn add_statm_to_graph(&mut self, kind: &StatementKind) {
        if let StatementKind::Assign(boxed_statm) = &kind {
            let dst = boxed_statm.0.local;
            let rvalue = &boxed_statm.1;
            match rvalue {
                Rvalue::Use(op) => {
                    self.add_operand(op, dst);
                    self.nodes[dst].op = NodeOp::Use;
                }
                Rvalue::CheckedBinaryOp(_, boxed_ops) => { //rustc version related
                    // 调用 add_operand 时不再需要可变借用 self
                    self.add_operand(&boxed_ops.0, dst);
                    self.add_operand(&boxed_ops.1, dst);
                    self.nodes[dst].op = NodeOp::CheckedBinaryOp
                },
                // todo: Aggregate Kind
                Rvalue::Aggregate(boxed_kind, ops) => {
                    for op in ops.iter() {
                        self.add_operand(op, dst);
                    }
                    self.nodes[dst].op = NodeOp::Aggregate;
                }
                _ => (),
                // _ => panic!("Error Rvalue!"),
            };
        }
    }

    fn add_terminator_to_graph(&mut self, kind: &TerminatorKind) {
        if let TerminatorKind::Call{func, args, destination, ..} = &kind {
            if let Operand::Constant(boxed_cnst) = func {
                if let Const::Val(_, ty) = boxed_cnst.const_ {
                    if let TyKind::FnDef(def_id, _) = ty.kind() {
                        let dst = destination.local;
                        for op in args.iter() { //rustc version related
                            self.add_operand(op, dst);
                        }
                        self.nodes[dst].op = NodeOp::Call(*def_id);
                        return;
                    }
                }
            }
            panic!("An error happened in add_terminator_to_graph.")
        }
    }

    fn to_dot_graph<'tcx>(&self, tcx: TyCtxt<'tcx>) {
        let mut dot = String::new();
        writeln!(dot, "digraph {} {{", tcx.def_path_str(self.def_id)).unwrap();
        writeln!(dot, "    node [shape=record];").unwrap();

        //nodes
        for (local, node) in self.nodes.iter_enumerated() {
            match node.op {
                NodeOp::Nop => {writeln!(dot, "    {:?} [label=\"<f0> {:?}\"]", local, local).unwrap();},
                NodeOp::Call(def_id) => {writeln!(dot, "    {:?} [label=\"<f0> {:?} | <f1> {} \"]", local, local, tcx.def_path_str(def_id)).unwrap();},
                _ => {writeln!(dot, "    {:?} [label=\"<f0> {:?} | <f1> {:?} \"]", local, local, node.op).unwrap();},
            };
        }

        //edges
        for edge in self.edges.iter() {
            writeln!(dot, "    {:?} -> {:?} [label=\"{:?}\"]", edge.src, edge.dst, edge.op).unwrap();
        }

        writeln!(dot, "}}").unwrap();

        println!("{}", dot);
    }
}

pub fn build_graph<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> Graph {
    let body: &Body = &tcx.optimized_mir(def_id);
    let mut graph = Graph::new(def_id, body.arg_count, body.local_decls.len());
    let basic_blocks = &body.basic_blocks;
    for basic_block_data in basic_blocks.iter() {
        for statement in basic_block_data.statements.iter() {
            graph.add_statm_to_graph(&statement.kind);
        }
        if let Some(terminator) = &basic_block_data.terminator {
            graph.add_terminator_to_graph(&terminator.kind);
        }
    }
    graph.to_dot_graph(tcx);
    graph
}

