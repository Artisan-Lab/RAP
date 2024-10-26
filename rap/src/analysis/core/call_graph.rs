pub mod call_graph_helper;
pub mod call_graph_visitor;

use call_graph_helper::CallGraphInfo;
use call_graph_visitor::CallGraphVisitor;
use rustc_middle::ty::TyCtxt;
use rustc_middle::mir::Body;
use rustc_hir::def::DefKind;

pub struct CallGraph<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub graph: CallGraphInfo,
}

impl<'tcx> CallGraph<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx: tcx,
            graph: CallGraphInfo::new(),
        }
    }

    pub fn start(&mut self) {
        for local_def_id in self.tcx.iter_local_def_id() {
            let hir_map = self.tcx.hir();
            if hir_map.maybe_body_owned_by(local_def_id).is_some() {
                let def_id = local_def_id.to_def_id();
                if self.tcx.is_mir_available(def_id) {
                    let def_kind = self.tcx.def_kind(def_id);
                    let body: &Body = match def_kind {
                        DefKind::Const | DefKind::Static{..} => {
                            // Compile Time Function Evaluation
                            &self.tcx.mir_for_ctfe(def_id)
                        },
                        _ => {
                            &self.tcx.optimized_mir(def_id) 
                        },
                    };
                    let mut call_graph_visitor =
                        CallGraphVisitor::new(self.tcx, def_id.into(), body, &mut self.graph);
                    call_graph_visitor.visit();
                }
            }
        }
        // for &def_id in self.tcx.mir_keys(()).iter() {
        //     if self.tcx.is_mir_available(def_id) {
        //         let body = &self.tcx.optimized_mir(def_id);
        //         let mut call_graph_visitor =
        //             CallGraphVisitor::new(self.tcx, def_id.into(), body, &mut self.graph);
        //         call_graph_visitor.visit();
        //     }
        // }
        self.graph.print_call_graph();
    }
}
