pub mod visitor;
pub mod contracts;

use crate::analysis::unsafety_isolation::hir_visitor::{ContainsUnsafe, RelatedFnCollector};
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

pub struct SenryxCheck<'tcx> {
    pub tcx: TyCtxt<'tcx>,
}

impl<'tcx> SenryxCheck<'tcx>{
    pub fn new(tcx: TyCtxt<'tcx>) -> Self{
        Self{
            tcx,
        }
    }

    pub fn start(&self) {
        let related_items = RelatedFnCollector::collect(self.tcx);
        let hir_map = self.tcx.hir();
        for (_, &ref vec) in &related_items {
            for (body_id, _span) in vec{
                let (function_unsafe, block_unsafe) = ContainsUnsafe::contains_unsafe(self.tcx, *body_id);
                let def_id = hir_map.body_owner_def_id(*body_id).to_def_id();
                if block_unsafe {
                    self.check_soundness(def_id);
                }
                if function_unsafe{
                    self.generate_safety_annotation(def_id);
                }
            }
        }
    }

    pub fn check_soundness(&self, def_id: DefId) {
        
        println!("Find unsound safe api, def_id: {:?}, location: {:?}, ",def_id, def_id);
    }

    pub fn generate_safety_annotation(&self, def_id: DefId) {
        
        println!("Annotate unsafe api, def_id: {:?}, location: {:?}, ",def_id, def_id);
    }
}