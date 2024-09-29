

//pub mod pointer_graph;
//pub mod constraint_solver;
//pub mod types;

use rustc_middle::ty::TyCtxt;


pub struct PointerAnalysis<'tcx> {
    pub tcx: TyCtxt<'tcx>,
}

impl<'tcx> PointerAnalysis<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self { tcx }
    }

    pub fn start(&self) {
        
            }
        }
  //  }
//}

/* 
use rustc_middle::mir::{Body, Operand, Place, Rvalue, StatementKind, TerminatorKind};
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_hir::def_id::DefId;

pub struct PointerAnalysis<'tcx> {
    pub tcx: TyCtxt<'tcx>,
}

impl<'tcx> PointerAnalysis<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self { tcx }
    }

    pub fn start(&self) {
        // 遍历所有的本地定义（函数/方法等）
        for local_def_id in self.tcx.iter_local_def_id() {
            let hir_map = self.tcx.hir();
            // 检查该定义是否有MIR
            if hir_map.maybe_body_owned_by(local_def_id).is_some() {
                let def_id = local_def_id.to_def_id();
                // 提取指针类型
                self.extract_pointer_types(def_id);
            }
        }
    }

    fn extract_pointer_types(&self, def_id: DefId) {
        // 获取函数体的MIR
        let body: &Body = self.tcx.optimized_mir(def_id);
        
        println!("Analyzing function: {:?}", self.tcx.def_path_str(def_id));

        // 遍历所有局部变量
        for local_decl in body.local_decls.iter() {
            let ty = local_decl.ty;
            if self.is_pointer_type(ty) {
                println!("Found pointer type in locals: {:?} in function {:?}", ty, self.tcx.def_path_str(def_id));
            }
        }

        // 遍历所有的基本块和语句
        for block in body.basic_blocks.iter() {
            for statement in block.statements.iter() {
                if let StatementKind::Assign(box (place, rvalue)) = &statement.kind {
                    self.check_rvalue_for_pointer(&place, &rvalue, def_id);
                }
            }

            // 检查终止条件
            if let Some(terminator) = &block.terminator {
                self.check_terminator_for_pointer(&terminator.kind, def_id);
            }
        }
    }

    fn is_pointer_type(&self, ty: Ty<'tcx>) -> bool {
        // 检查类型是否是引用或裸指针类型
        matches!(ty.kind(), TyKind::Ref(..) | TyKind::RawPtr(..))
    }

    fn check_rvalue_for_pointer(&self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, def_id: DefId) {
    // 获取函数体的MIR
    let body: &Body = self.tcx.optimized_mir(def_id);
    
    // 将局部声明传递给 `place.ty`
    let ty = place.ty(&body.local_decls, self.tcx).ty;
    
    if self.is_pointer_type(ty) {
        println!("Pointer type in assignment: {:?} in function {:?}", ty, self.tcx.def_path_str(def_id));
    }
}

fn check_terminator_for_pointer(&self, terminator: &TerminatorKind<'tcx>, def_id: DefId) {
    // 检查终止条件中的指针类型
    match terminator {
        TerminatorKind::Call { args, .. } => {
            for arg in args {
                if let Operand::Copy(place) | Operand::Move(place) = arg {
                    // 获取函数体的MIR
                    let body: &Body = self.tcx.optimized_mir(def_id);
                    
                    // 使用局部声明获取类型
                    let ty = place.ty(&body.local_decls, self.tcx).ty;
                    if self.is_pointer_type(ty) {
                        println!("Pointer type in function call: {:?} in function {:?}", ty, self.tcx.def_path_str(def_id));
                    }
                }
            }
        }
        _ => {}
    }
}
}
*/