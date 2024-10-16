pub mod contracts;
pub mod matcher;
pub mod visitor;

use crate::analysis::unsafety_isolation::{
    hir_visitor::{ContainsUnsafe, RelatedFnCollector},
    UnsafetyIsolationCheck,
};
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use visitor::BodyVisitor;

pub struct SenryxCheck<'tcx> {
    pub tcx: TyCtxt<'tcx>,
}

impl<'tcx> SenryxCheck<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self { tcx }
    }

    pub fn start(&self) {
        let related_items = RelatedFnCollector::collect(self.tcx);
        let hir_map = self.tcx.hir();
        for (_, &ref vec) in &related_items {
            for (body_id, _span) in vec {
                let (function_unsafe, block_unsafe) =
                    ContainsUnsafe::contains_unsafe(self.tcx, *body_id);
                let def_id = hir_map.body_owner_def_id(*body_id).to_def_id();
                if block_unsafe {
                    self.check_soundness(def_id);
                }
                if function_unsafe {
                    self.annotate_safety(def_id);
                }
            }
        }
    }

    pub fn check_soundness(&self, def_id: DefId) {
        self.pre_handle_type(def_id);
        println!(
            "Find unsound safe api, def_id: {:?}, location: {:?}, ",
            def_id, def_id
        );
    }

    pub fn annotate_safety(&self, def_id: DefId) {
        self.pre_handle_type(def_id);
        println!(
            "Annotate unsafe api, def_id: {:?}, location: {:?}, ",
            def_id, def_id
        );
    }

    pub fn pre_handle_type(&self, def_id: DefId) {
        let mut uig_checker = UnsafetyIsolationCheck::new(self.tcx);
        let func_type = uig_checker.get_type(def_id);
        let mut body_visitor = BodyVisitor::new(self.tcx, def_id);
        if func_type == 1 {
            let func_cons = uig_checker.search_constructor(def_id);
            for func_con in func_cons {
                let mut cons_body_visitor = BodyVisitor::new(self.tcx, func_con);
                cons_body_visitor.path_forward_check();
                // TODO: cache fields' states

                // TODO: update method body's states

                // analyze body's states
                body_visitor.path_forward_check();
            }
        } else {
            body_visitor.path_forward_check();
        }
    }
}
