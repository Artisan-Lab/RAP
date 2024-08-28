use rustc_index::IndexVec;
use rustc_middle::mir::Body;
use rustc_middle::mir::StatementKind::Assign;
use rustc_middle::mir::Local;
use rustc_middle::ty::TyCtxt;
use rustc_hir::def_id::DefId;

#[derive(Clone)]
pub enum DataFlowOp {
    Nop,
}
#[derive(Clone)]
pub struct VariableNode {
    op: DataFlowOp,
    parents: Vec<Local>,
}

impl VariableNode {
    pub fn new() -> Self {
        Self { op: DataFlowOp::Nop, parents: Vec::new() }
    }

    pub fn new_with(op: DataFlowOp, parents: Vec<Local>) -> Self {
        Self {
            op, parents
        }
    }
}

pub struct ImmNode {}

#[derive(Clone)]
pub enum DataFlowGraphNode {
    VariableNode(VariableNode),
    ImmNode,
}

pub type DataFlowGraph = IndexVec<Local, DataFlowGraphNode>;

pub fn build_graph<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> DataFlowGraph {
    let body: &Body = &tcx.optimized_mir(def_id);
    let graph = DataFlowGraph::from_elem(DataFlowGraphNode::VariableNode(VariableNode::new()), &body.local_decls);
    let basic_blocks = &body.basic_blocks;
    for basic_block_data in basic_blocks.iter() {
        for statement in basic_block_data.statements.iter() {
            if let Assign(statm) = &statement.kind {
                let local = &statm.0.local;
                let rvalue = &statm.1;
                println!("{:?}", local);
            }
        }
    }
    graph
}

