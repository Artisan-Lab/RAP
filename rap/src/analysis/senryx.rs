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
                    self.annotate_safety(def_id);
                }
            }
        }
    }

    pub fn check_soundness(&self, def_id: DefId) {
        
        println!("Find unsound safe api, def_id: {:?}, location: {:?}, ",def_id, def_id);
    }

    pub fn annotate_safety(&self, def_id: DefId) {
        
        println!("Annotate unsafe api, def_id: {:?}, location: {:?}, ",def_id, def_id);
    }

    //retval: 0-constructor, 1-method, 2-function
    pub fn get_type(&self,def_id: DefId) -> usize{
        let tcx = self.tcx;
        let mut node_type = 2;
        if let Some(assoc_item) = tcx.opt_associated_item(def_id) {
            if assoc_item.fn_has_self_parameter {
                node_type = 1;
            } else if !assoc_item.fn_has_self_parameter  {
                let fn_sig = tcx.fn_sig(def_id).skip_binder();
                let output = fn_sig.output().skip_binder();
                if output.is_param(0) {
                    node_type = 0;
                }
                if let Some(assoc_item) = tcx.opt_associated_item(def_id) {
                    if let Some(impl_id) = assoc_item.impl_container(tcx) {
                        let ty = tcx.type_of(impl_id).skip_binder();
                        if output == ty{
                            node_type = 0;
                        }
                    }
                }
            }
        }
        return node_type;
    }
}