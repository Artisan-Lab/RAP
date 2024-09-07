pub mod graph;

use std::collections::HashMap;

use rustc_middle::mir::Body;
use rustc_middle::ty::TyCtxt;
use rustc_hir::def_id::DefId;

use graph::Graph;

pub struct DataFlow<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub graphs: HashMap<DefId, Graph>
}

impl<'tcx> DataFlow<'tcx> {
    pub fn new(tcx : TyCtxt<'tcx>) -> Self {
        Self { tcx: tcx, graphs: HashMap::new() }
    }

    pub fn start(&mut self) {
        for local_def_id in self.tcx.iter_local_def_id() {
            let hir_map = self.tcx.hir();
            if hir_map.maybe_body_owned_by(local_def_id).is_some() {
                let def_id = local_def_id.to_def_id();
                let graph = self.build_graph(def_id);
                self.graphs.insert(def_id, graph);
            }
        }
    }

    fn build_graph(&self, def_id: DefId) -> Graph {
        let body: &Body = self.tcx.optimized_mir(def_id);
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
        graph
    }
}



