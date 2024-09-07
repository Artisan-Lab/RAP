use rustc_middle::ty;
use rustc_middle::ty::{Ty, TyCtxt};
use super::corner_handle::is_corner_adt;

#[derive(PartialEq,Debug,Copy,Clone)]
pub enum TyKind {
    Adt,
    RawPtr,
    Tuple,
    CornerCase,
    Ref,
}

pub fn kind<'tcx>(current_ty: Ty<'tcx>) -> TyKind {
    match current_ty.kind() {
        ty::RawPtr(..) => TyKind::RawPtr,
        ty::Ref(..) => TyKind::Ref,
        ty::Tuple(..) => TyKind::Tuple,
        ty::Adt(ref adt_def, _) => {
            if is_corner_adt(format!("{:?}", adt_def)) {
                return TyKind::CornerCase;
            }
            else{
                return TyKind::Adt;
            }
        },
        _ => TyKind::Adt,
    }
}

pub fn is_not_drop<'tcx>(tcx: TyCtxt<'tcx>, current_ty: Ty<'tcx>) -> bool {
    match current_ty.kind() {
        ty::Bool
        | ty::Char
        | ty::Int(_)
        | ty::Uint(_)
        | ty::Float(_) => true,
        ty::Array(ref tys,_) => is_not_drop(tcx, *tys),
        ty::Adt(ref adtdef, ref substs) => {
            for field in adtdef.all_fields() {
                if !is_not_drop(tcx, field.ty(tcx, substs)) {
                    return false;
                }
            }
            true
        },
        ty::Tuple(ref tuple_fields) => {
            for tys in tuple_fields.iter() {
                if !is_not_drop(tcx, tys) {
                    return false;
                }
            }
            true
        },
        _ => false,
    }
}
