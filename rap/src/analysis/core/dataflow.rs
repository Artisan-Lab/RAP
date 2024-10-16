pub mod debug;
pub mod graph;

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::process::Command;

use rustc_hir::def_id::DefId;
use rustc_middle::mir::Body;
use rustc_middle::ty::TyCtxt;

use graph::Graph;

pub struct DataFlow<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub graphs: HashMap<DefId, Graph>,
    pub debug: bool,
}

impl<'tcx> DataFlow<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, debug: bool) -> Self {
        Self {
            tcx: tcx,
            graphs: HashMap::new(),
            debug,
        }
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
        if self.debug {
            self.draw_graphs();
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

    pub fn draw_graphs(&self) {
        let dir_name = "DataflowGraph";

        Command::new("rm")
            .args(&["-rf", dir_name])
            .output()
            .expect("Failed to remove directory.");

        Command::new("mkdir")
            .args(&[dir_name])
            .output()
            .expect("Failed to create directory.");

        for (def_id, graph) in self.graphs.iter() {
            let name = self.tcx.def_path_str(def_id);
            let dot_file_name = format!("DataflowGraph/{}.dot", &name);
            let png_file_name = format!("DataflowGraph/{}.png", &name);
            let mut file = File::create(&dot_file_name).expect("Unable to create file.");
            let dot = graph.to_dot_graph(&self.tcx);
            file.write_all(dot.as_bytes())
                .expect("Unable to write data.");

            Command::new("dot")
                .args(&["-Tpng", &dot_file_name, "-o", &png_file_name])
                .output()
                .expect("Failed to execute Graphviz dot command.");
        }
    }
}
