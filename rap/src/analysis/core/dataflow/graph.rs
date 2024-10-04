use std::cell::Cell;
use std::collections::HashSet;
use std::fmt::Write;

use rustc_index::IndexVec;
use rustc_middle::mir::{TerminatorKind, StatementKind, Operand, Rvalue, Local, Const, BorrowKind, AggregateKind, Mutability, PlaceElem, Place};
use rustc_middle::ty::{TyKind, TyCtxt};
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
    Aggregate(AggKind),
    ShallowInitBox,
    CopyForDeref,
    //TerminatorKind
    Call(DefId)
}

#[derive(Clone, Debug)]
pub enum EdgeOp {
    Nop,
    //Operand
    Move,
    Copy,
    Const,
    //Mutability
    Immut,
    Mut,
    //Place
    Deref,
    Field(String),
    Downcast(String),
}

#[derive(Clone)]
pub enum GraphEdge {
    NodeEdge{
        src: Local,
        dst: Local,
        op: EdgeOp,
    },
    ConstEdge{
        src: String,
        dst: Local,
        op: EdgeOp,
    }
}

impl GraphEdge {
    pub fn to_dot_graph<'tcx> (&self) -> String {
        fn escaped_string(s: String) -> String{
            s
                .replace("{", "\\{")
                .replace("}", "\\}")
                .replace("<", "\\<")
                .replace(">", "\\>")
                .replace("\"", "\\\"")
        }

        let mut attr = String::new();
        let mut dot = String::new();
        match self { //label=xxx
            GraphEdge::NodeEdge { src:_, dst:_, op } => {write!(attr, "label=\"{}\" ", escaped_string(format!("{:?}", op))).unwrap();},
            GraphEdge::ConstEdge { src:_, dst:_, op } => {write!(attr, "label=\"{}\" ", escaped_string(format!("{:?}", op))).unwrap();},
        }
        match self {
            GraphEdge::NodeEdge { src, dst, op:_ } => {write!(dot, "{:?} -> {:?} [{}]", src, dst, attr).unwrap();},
            GraphEdge::ConstEdge { src, dst, op:_ } => {write!(dot, "{:?} -> {:?} [{}]", src, dst, attr).unwrap();},
        }
        dot
    }
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

    pub fn to_dot_graph<'tcx> (&self, tcx: &TyCtxt<'tcx>, local: Local, color: Option<String>, is_marker: bool) -> String {
        fn escaped_string(s: String) -> String{
            s
                .replace("{", "\\{")
                .replace("}", "\\}")
                .replace("<", "\\<")
                .replace(">", "\\>")
        }

        let mut attr = String::new();
        let mut dot = String::new();
        match self.op { //label=xxx
            NodeOp::Nop => {
                if is_marker{
                    write!(attr, "label=\"\" style=dashed ").unwrap();
                }
                else {
                    write!(attr, "label=\"<f0> {:?}\" ", local).unwrap();
                }
            },
            NodeOp::Call(def_id) => {
                let func_name = tcx.def_path_str(def_id);
                if is_marker {
                    write!(attr, "label=\"fn {}\" style=dashed ", escaped_string(func_name)).unwrap();
                }
                else {
                    write!(attr, "label=\"<f0> {:?} | <f1> fn {}\" ", local, escaped_string(func_name)).unwrap();
                }
            },
            NodeOp::Aggregate(agg_kind) => {
                match agg_kind {
                    AggKind::Adt(def_id) => {
                        let agg_name = format!("{}::{{..}}", tcx.def_path_str(def_id));
                        if is_marker {
                            write!(attr, "label=\"Agg {}\" style=dashed ", escaped_string(agg_name)).unwrap();
                        }
                        else {
                            write!(attr, "label=\"<f0> {:?} | <f1> Agg {}\" ", local, escaped_string(agg_name)).unwrap();
                        }
                    },
                    _ => {
                        if is_marker {
                            write!(attr, "label=\"{:?}\" style=dashed ", agg_kind).unwrap();
                        }
                        else {
                            write!(attr, "label=\"<f0> {:?} | {:?}\" ", local, agg_kind).unwrap();
                        }
                    }
                }
            }
            _ => {
                if is_marker {
                    write!(attr, "label=\"<f1> {:?}\" style=dashed ", self.op).unwrap();
                }
                else {
                    write!(attr, "label=\"<f0> {:?} | <f1> {:?}\" ", local, self.op).unwrap();
                }
            },
        };
        match color { //color=xxx
            None => {},
            Some(color) => {write!(attr, "color={} ", color).unwrap();},
        }
        write!(dot, "{:?} [{}]", local, attr).unwrap();
        dot
    }
}

pub type EdgeIdx = usize;
pub type GraphNodes = IndexVec<Local, GraphNode>;
pub type GraphEdges = IndexVec<EdgeIdx, GraphEdge>;
pub struct Graph {
    pub def_id: DefId,
    pub argc: usize,
    pub nodes: GraphNodes, //constsis of locals in mir and newly created markers
    pub edges: GraphEdges,
    pub n_locals: usize,
}

impl Graph {
    pub fn new(def_id: DefId, argc: usize, n_locals: usize) -> Self {
        Self { def_id, argc, nodes: GraphNodes::from_elem_n(GraphNode::new(), n_locals), edges: GraphEdges::new(), n_locals }
    }

    pub fn add_node_edge(&mut self, src: Local, dst: Local, op: EdgeOp) -> EdgeIdx {
        let edge_idx = self.edges.push(GraphEdge::NodeEdge {src, dst, op});
        self.nodes[dst].in_edges.push(edge_idx);
        self.nodes[src].out_edges.push(edge_idx);
        edge_idx
    }

    pub fn add_const_edge(&mut self, src: String, dst: Local, op: EdgeOp) -> EdgeIdx {
        let edge_idx = self.edges.push(GraphEdge::ConstEdge {src, dst, op});
        self.nodes[dst].in_edges.push(edge_idx);
        edge_idx
    }

    pub fn add_operand(&mut self, operand: &Operand, dst: Local) {
        match operand {
            Operand::Copy(place) => {
                let src = self.parse_place(place);
                self.add_node_edge(src, dst, EdgeOp::Copy);
            },
            Operand::Move(place) => {
                let src = self.parse_place(place);
                self.add_node_edge(src, dst, EdgeOp::Move);
            },
            Operand::Constant(boxed_const_op) => {
                self.add_const_edge(boxed_const_op.const_.to_string(), dst, EdgeOp::Const);
            }
        }
    }

    pub fn parse_place(&mut self, place: &Place) -> Local {
        fn parse_one_step(graph: &mut Graph, src: Local, place_elem: PlaceElem) -> Local{
            let dst = graph.nodes.push(GraphNode::new());
            match place_elem {
                PlaceElem::Deref => {
                    graph.add_node_edge(src, dst, EdgeOp::Deref);
                },
                PlaceElem::Field(field_idx, _) => {
                    graph.add_node_edge(src, dst, EdgeOp::Field(format!("{:?}", field_idx)));
                },
                PlaceElem::Downcast(symbol, _) => {
                    graph.add_node_edge(src, dst, EdgeOp::Downcast(symbol.unwrap().to_string()));
                },
                _ => { todo!() }
            }
            dst
        }
        let mut ret = place.local;
        for place_elem in place.projection {
            ret = parse_one_step(self, ret, place_elem);
        }
        ret
    }

    pub fn add_statm_to_graph(&mut self, kind: &StatementKind) {
        if let StatementKind::Assign(boxed_statm) = &kind {
            let place = boxed_statm.0;
            let dst = self.parse_place(&place);
            let rvalue = &boxed_statm.1;
            match rvalue {
                Rvalue::Use(op) => {
                    self.add_operand(op, dst);
                    self.nodes[dst].op = NodeOp::Use;
                },
                Rvalue::Repeat(op, _) => {
                    self.add_operand(op, dst);
                    self.nodes[dst].op = NodeOp::Repeat;
                },
                Rvalue::Ref(_, borrow_kind, place) => {
                    let op = match borrow_kind {
                        BorrowKind::Shared => EdgeOp::Immut,
                        BorrowKind::Mut {..} => EdgeOp::Mut,
                        BorrowKind::Shallow => {panic!("Unimplemented BorrowKind!")}
                    };
                    let src = self.parse_place(place);
                    self.add_node_edge(src, dst, op);
                    self.nodes[dst].op = NodeOp::Ref;
                },
                Rvalue::AddressOf(mutability, place) => {
                    let op = match mutability {
                        Mutability::Not => EdgeOp::Immut,
                        Mutability::Mut => EdgeOp::Mut,
                    };
                    let src = self.parse_place(place);
                    self.add_node_edge(src, dst, op);
                    self.nodes[dst].op = NodeOp::AddressOf;
                },
                Rvalue::Len(place) => {
                    let src = self.parse_place(place);
                    self.add_node_edge(src, dst, EdgeOp::Nop);
                    self.nodes[dst].op = NodeOp::Len;

                },
                Rvalue::Cast(_cast_kind, operand, _) => {
                    self.add_operand(operand, dst);
                    self.nodes[dst].op = NodeOp::Cast;
                },
                Rvalue::BinaryOp(_, operands) => {
                    self.add_operand(&operands.0, dst);
                    self.add_operand(&operands.1, dst);
                    self.nodes[dst].op = NodeOp::CheckedBinaryOp;
                },
                Rvalue::CheckedBinaryOp(_, operands) => {
                    self.add_operand(&operands.0, dst);
                    self.add_operand(&operands.1, dst);
                    self.nodes[dst].op = NodeOp::CheckedBinaryOp;
                },
                Rvalue::Aggregate(boxed_kind, operands) => {
                    for operand in operands.iter() {
                        self.add_operand(operand, dst);
                    }
                    match **boxed_kind {
                        AggregateKind::Array(_) => { self.nodes[dst].op = NodeOp::Aggregate(AggKind::Array) },
                        AggregateKind::Tuple => { self.nodes[dst].op = NodeOp::Aggregate(AggKind::Tuple) },
                        AggregateKind::Adt(def_id, ..) => { self.nodes[dst].op = NodeOp::Aggregate(AggKind::Adt(def_id)) },
                        _ => todo!()
                    }
                },
                Rvalue::UnaryOp(_, operand) => {
                    self.add_operand(operand, dst);
                    self.nodes[dst].op = NodeOp::UnaryOp;
                },
                Rvalue::NullaryOp(_, ty) => {
                    self.add_const_edge(ty.to_string(), dst, EdgeOp::Nop);
                    self.nodes[dst].op = NodeOp::NullaryOp;
                },
                Rvalue::ThreadLocalRef(_) => {todo!()},
                Rvalue::Discriminant(place) => {
                    let src = self.parse_place(place);
                    self.add_node_edge(src, dst, EdgeOp::Nop);
                    self.nodes[dst].op = NodeOp::Discriminant;
                },
                Rvalue::ShallowInitBox(operand, _) => {
                    self.add_operand(operand, dst);
                    self.nodes[dst].op = NodeOp::ShallowInitBox;
                },
                Rvalue::CopyForDeref(place) => {
                    let src = self.parse_place(place);
                    self.add_node_edge(src, dst, EdgeOp::Nop);
                    self.nodes[dst].op = NodeOp::CopyForDeref;
                },
            };
        }
    }

    pub fn add_terminator_to_graph(&mut self, kind: &TerminatorKind) {
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

    pub fn to_dot_graph<'tcx>(&self, tcx: &TyCtxt<'tcx>) -> String {
        let mut dot = String::new();
        let name = tcx.def_path_str(self.def_id);

        writeln!(dot, "digraph \"{}\" {{", &name).unwrap();
        writeln!(dot, "    node [shape=record];").unwrap();
        for (local, node) in self.nodes.iter_enumerated() {
            let node_dot = if local <= Local::from_usize(self.argc) {
                node.to_dot_graph(tcx, local, Some(String::from("red")), false)
            }
            else if local <= Local::from_usize(self.n_locals) {
                node.to_dot_graph(tcx, local, None, false)
            }
            else {
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

    pub fn collect_equivalent_locals(&self, local: Local) -> HashSet<Local> {
        let mut set = HashSet::new();
        let mut node_operator = |idx: Local| -> bool {
            let node = &self.nodes[idx];
            match node.op {
                NodeOp::Nop | NodeOp::Use | NodeOp::Ref => { //Nop means a orphan node or a parameter
                    set.insert(idx);
                    true
                },
                _ => false,
            }
        };
        let mut edge_validator = |op: &EdgeOp| -> bool {
            match op {
                EdgeOp::Copy | EdgeOp::Move | EdgeOp::Mut | EdgeOp::Immut => true,
                EdgeOp::Nop | EdgeOp::Const | EdgeOp::Deref | EdgeOp::Downcast(_) | EdgeOp::Field(_) => false
            }
        };
        self.dfs(local, Direction::Upside, &mut node_operator, &mut edge_validator);
        self.dfs(local, Direction::Downside, &mut node_operator, &mut edge_validator);
        set
    }

    pub fn is_connected(&self, idx_1: Local, idx_2: Local) -> bool {
        let target = idx_2;
        let find = Cell::new(false);
        let mut node_operator = |idx: Local| -> bool {
            find.set(idx == target);
            !find.get() // if not found, move on
        };
        let mut edge_validator = |_: &EdgeOp| -> bool {
            true
        };
        self.dfs(idx_1, Direction::Downside, &mut node_operator, &mut edge_validator);
        if !find.get() {
            self.dfs(idx_1, Direction::Upside, &mut node_operator, &mut edge_validator);
        }
        find.get()
    }

    pub fn param_return_deps(&self) -> IndexVec<Local, bool> { //the length is argc + 1, because _0 depends on _0 itself.
        let _0 = Local::from_usize(0);
        let deps = (0..self.argc + 1).map(|i| {
            let _i = Local::from_usize(i);
            self.is_connected(_i, _0)
        }).collect();
        deps
    }

    pub fn dfs<F, G>(&self, now: Local, direction: Direction, node_operator: &mut F, edge_validator: &mut G)
    where 
        F: FnMut(Local) -> bool,
        G: FnMut(&EdgeOp) -> bool,
    {
        if node_operator(now) {
            match direction {
                Direction::Upside => {
                    for edge_idx in self.nodes[now].in_edges.iter() {
                        let edge = &self.edges[*edge_idx];
                        if let GraphEdge::NodeEdge { src, op, .. } = edge {
                            if edge_validator(op) {
                                self.dfs(*src, direction, node_operator, edge_validator);
                            }
                        }
                    }
                },
                Direction::Downside => {
                    for edge_idx in self.nodes[now].out_edges.iter() {
                        let edge = &self.edges[*edge_idx];
                        if let GraphEdge::NodeEdge { op, dst, .. } = edge {
                            if edge_validator(op) {
                                self.dfs(*dst, direction, node_operator, edge_validator);
                            }
                        }
                    }
                }
            };
        }
    }
}

#[derive(Clone, Copy)]
pub enum Direction {
    Upside, Downside
}

#[derive(Clone, Copy, Debug)]
pub enum AggKind {
    Array,
    Tuple,
    Adt(DefId),
}