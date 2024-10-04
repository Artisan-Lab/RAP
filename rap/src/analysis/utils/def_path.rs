use rustc_middle::ty::TyCtxt;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{CrateNum, DefId, LocalDefId, LOCAL_CRATE};
use rustc_hir::PrimTy;
use rustc_span::symbol::{Ident, Symbol};
use rustc_middle::ty::fast_reject::SimplifiedType;
use rustc_hir::{ImplItemRef, ItemKind, Mutability, Node, OwnerId, TraitItemRef};
use rustc_middle::ty::{FloatTy, IntTy, UintTy};

pub fn def_path_last_def_id<'tcx>(tcx: &TyCtxt<'tcx>, path: &[&str]) -> DefId {
    def_path_def_ids(tcx, path).last().unwrap()
}


// The code below refers to rust-clippy.

/// Resolves a def path like `std::vec::Vec` to its [`DefId`]s, see [`def_path_res`].
pub fn def_path_def_ids<'tcx>(tcx: &TyCtxt<'tcx>, path: &[&str]) -> impl Iterator<Item = DefId> {
    def_path_res(tcx, path).into_iter().filter_map(|res| res.opt_def_id())
}


pub fn def_path_res<'tcx>(tcx: &TyCtxt<'tcx>, path: &[&str]) -> Vec<Res> {
    let (base, mut path) = match *path {
        [primitive] => {
            return vec![PrimTy::from_name(Symbol::intern(primitive)).map_or(Res::Err, Res::PrimTy)];
        },
        [base, ref path @ ..] => (base, path),
        _ => return Vec::new(),
    };

    let base_sym = Symbol::intern(base);

    let local_crate = if tcx.crate_name(LOCAL_CRATE) == base_sym {
        Some(LOCAL_CRATE.as_def_id())
    } else {
        None
    };

    let starts = find_primitive_impls(tcx, base)
        .chain(tcx.crates(())
            .iter()
            .copied()
            .filter(move |&num| tcx.crate_name(num) == base_sym)
            .map(CrateNum::as_def_id)
        )
        .chain(local_crate)
        .map(|id| Res::Def(tcx.def_kind(id), id));

    let mut resolutions: Vec<Res> = starts.collect();

    while let [segment, rest @ ..] = path {
        path = rest;
        let segment = Symbol::intern(segment);

        resolutions = resolutions
            .into_iter()
            .filter_map(|res| res.opt_def_id())
            .flat_map(|def_id| {
                // When the current def_id is e.g. `struct S`, check the impl items in
                // `impl S { ... }`
                let inherent_impl_children = tcx
                    .inherent_impls(def_id)
                    .iter()
                    .flat_map(|&impl_def_id| item_children_by_name(tcx, impl_def_id, segment));

                let direct_children = item_children_by_name(tcx, def_id, segment);

                inherent_impl_children.chain(direct_children)
            })
            .collect();
    }

    resolutions
}

fn find_primitive_impls<'tcx>(tcx: &TyCtxt<'tcx>, name: &str) -> impl Iterator<Item = DefId> + 'tcx {
    let ty = match name {
        "bool" => SimplifiedType::Bool,
        "char" => SimplifiedType::Char,
        "str" => SimplifiedType::Str,
        "array" => SimplifiedType::Array,
        "slice" => SimplifiedType::Slice,
        // FIXME: rustdoc documents these two using just `pointer`.
        //
        // Maybe this is something we should do here too.
        "const_ptr" => SimplifiedType::Ptr(Mutability::Not),
        "mut_ptr" => SimplifiedType::Ptr(Mutability::Mut),
        "isize" => SimplifiedType::Int(IntTy::Isize),
        "i8" => SimplifiedType::Int(IntTy::I8),
        "i16" => SimplifiedType::Int(IntTy::I16),
        "i32" => SimplifiedType::Int(IntTy::I32),
        "i64" => SimplifiedType::Int(IntTy::I64),
        "i128" => SimplifiedType::Int(IntTy::I128),
        "usize" => SimplifiedType::Uint(UintTy::Usize),
        "u8" => SimplifiedType::Uint(UintTy::U8),
        "u16" => SimplifiedType::Uint(UintTy::U16),
        "u32" => SimplifiedType::Uint(UintTy::U32),
        "u64" => SimplifiedType::Uint(UintTy::U64),
        "u128" => SimplifiedType::Uint(UintTy::U128),
        "f32" => SimplifiedType::Float(FloatTy::F32),
        "f64" => SimplifiedType::Float(FloatTy::F64),
        _ => return [].iter().copied(),
    };

    tcx.incoherent_impls(ty).iter().copied()
}

fn non_local_item_children_by_name<'tcx>(tcx: &TyCtxt<'tcx>, def_id: DefId, name: Symbol) -> Vec<Res> {
    match tcx.def_kind(def_id) {
        DefKind::Mod | DefKind::Enum | DefKind::Trait => tcx
            .module_children(def_id)
            .iter()
            .filter(|item| item.ident.name == name)
            .map(|child| child.res.expect_non_local())
            .collect(),
        DefKind::Impl { .. } => tcx
            .associated_item_def_ids(def_id)
            .iter()
            .copied()
            .filter(|assoc_def_id| tcx.item_name(*assoc_def_id) == name)
            .map(|assoc_def_id| Res::Def(tcx.def_kind(assoc_def_id), assoc_def_id))
            .collect(),
        _ => Vec::new(),
    }
}

fn local_item_children_by_name<'tcx>(tcx: &TyCtxt<'tcx>, local_id: LocalDefId, name: Symbol) -> Vec<Res> {
    let hir = tcx.hir();

    let root_mod;
    let item_kind = match hir.find_by_def_id(local_id) {
        Some(Node::Crate(r#mod)) => {
            root_mod = ItemKind::Mod(r#mod);
            &root_mod
        },
        Some(Node::Item(item)) => &item.kind,
        _ => return Vec::new(),
    };

    let res = |ident: Ident, owner_id: OwnerId| {
        if ident.name == name {
            let def_id = owner_id.to_def_id();
            Some(Res::Def(tcx.def_kind(def_id), def_id))
        } else {
            None
        }
    };

    match item_kind {
        ItemKind::Mod(r#mod) => r#mod
            .item_ids
            .iter()
            .filter_map(|&item_id| res(hir.item(item_id).ident, item_id.owner_id))
            .collect(),
        ItemKind::Impl(r#impl) => r#impl
            .items
            .iter()
            .filter_map(|&ImplItemRef { ident, id, .. }| res(ident, id.owner_id))
            .collect(),
        ItemKind::Trait(.., trait_item_refs) => trait_item_refs
            .iter()
            .filter_map(|&TraitItemRef { ident, id, .. }| res(ident, id.owner_id))
            .collect(),
        _ => Vec::new(),
    }
}

fn item_children_by_name<'tcx>(tcx: &TyCtxt<'tcx>, def_id: DefId, name: Symbol) -> Vec<Res> {
    if let Some(local_id) = def_id.as_local() {
        local_item_children_by_name(tcx, local_id, name)
    } else {
        non_local_item_children_by_name(tcx, def_id, name)
    }
}