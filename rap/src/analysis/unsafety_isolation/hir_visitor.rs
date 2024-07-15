use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{
    def_id::{DefId, LocalDefId},
    intravisit,
    intravisit::Visitor,
    Block, BodyId, Body, HirId, Impl, ItemKind,Expr,ExprKind,
};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::Span;

/// Maps `HirId` of a type to `BodyId` of related impls.
pub type RelatedItemMap = FxHashMap<Option<HirId>, Vec<(BodyId, Span)>>;

pub struct RelatedFnCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    hash_map: RelatedItemMap,
}

impl<'tcx> RelatedFnCollector<'tcx> {
    pub fn collect(tcx: TyCtxt<'tcx>) -> RelatedItemMap {
        let mut collector = RelatedFnCollector {
            tcx,
            hash_map: RelatedItemMap::default(),
        };

        tcx.hir().visit_all_item_likes_in_crate(&mut collector);

        collector.hash_map
    }
}

impl<'tcx> Visitor<'tcx> for RelatedFnCollector<'tcx> {
    fn visit_item(&mut self, item: &'tcx rustc_hir::Item<'tcx>) {
        let hir_map = self.tcx.hir();
        //println!("{:?}",&item.kind);
        match &item.kind {
            ItemKind::Impl(Impl {
                unsafety: _unsafety,
                generics: _generics,
                self_ty,
                items: impl_items,
                ..
            }) => {
                let key = Some(self_ty.hir_id);
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.extend(impl_items.iter().filter_map(|impl_item_ref| {
                    let hir_id = impl_item_ref.id.hir_id();
                    hir_map
                        .maybe_body_owned_by(hir_id.owner.def_id)
                        .map(|body_id| (body_id, impl_item_ref.span))
                }));
            }
            // Free-standing (top level) functions and default trait impls have `None` as a key.
            ItemKind::Trait(_is_auto, _unsafety, _generics, _generic_bounds, trait_items) => {
                let key = None;
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.extend(trait_items.iter().filter_map(|trait_item_ref| {
                    let hir_id = trait_item_ref.id.hir_id();
                    hir_map
                        .maybe_body_owned_by(hir_id.owner.def_id)
                        .map(|body_id| (body_id, trait_item_ref.span))
                }));
            }
            ItemKind::Fn(_fn_sig, _generics, body_id) => {
                let key = Some(body_id.hir_id);
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.push((*body_id, item.span));
            }
            _ => (),
        }
    }

    fn visit_trait_item(&mut self, _trait_item: &'tcx rustc_hir::TraitItem<'tcx>) {
        // We don't process items inside trait blocks
    }

    fn visit_impl_item(&mut self, _impl_item: &'tcx rustc_hir::ImplItem<'tcx>) {
        // We don't process items inside impl blocks
    }

    fn visit_foreign_item(&mut self, _foreign_item: &'tcx rustc_hir::ForeignItem<'tcx>) {
        // We don't process foreign items
    }
}


pub struct ContainsUnsafe<'tcx> {
    tcx: TyCtxt<'tcx>,
    function_unsafe: bool,
    block_unsafe: bool,
}

impl<'tcx> ContainsUnsafe<'tcx> {
    /// Given a `BodyId`, returns if the corresponding body contains unsafe code in it.
    /// Note that it only checks the function body, so this function will return false for
    /// body ids of functions that are defined as unsafe.
    pub fn contains_unsafe(tcx: TyCtxt<'tcx>, body_id: BodyId) -> (bool,bool) {
        let mut visitor = ContainsUnsafe {
            tcx,
            function_unsafe: false,
            block_unsafe: false,
        };

        let body = visitor.tcx.hir().body(body_id);
        visitor.function_unsafe = visitor.body_unsafety(&body);
        visitor.visit_body(body);

        (visitor.function_unsafe, visitor.block_unsafe)
    }

    fn body_unsafety(&self, body: &'tcx Body<'tcx>) -> bool {
        // self.contains_unsafe 
        let did = body.value.hir_id.owner.to_def_id();
        let sig = self.tcx.fn_sig(did);
        if let rustc_hir::Unsafety::Unsafe = sig.skip_binder().unsafety(){
            return true
        }
        false
    }

    pub fn extract_expr_calls(expr_kind: &rustc_hir::ExprKind<'_>) {
        match expr_kind {
            ExprKind::Call(_func,_) => {
                // if let ExprKind::Path(qpath) = &func.kind {
                //     if let QPath::Resolved(_,path) = qpath {
                //         if let Some(_) = path.segments.last() {
                //             let function_name = path
                //                                 .segments
                //                                 .iter()
                //                                 .map(|segment| segment.ident.to_string())
                //                                 .collect::<Vec<String>>()
                //                                 .join("::");
                //             //todo:cache
                //             // println!("{:?}",function_name);
                //         }
                //     }
                // }
            }
            ExprKind::MethodCall(_path, _, _, _) => {
                
            }
            // ExprKind::Array([expr]) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Tup([expr]) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Binary(_, expr1, expr2) => {
            //     Self::extract_expr_calls(&expr1.kind);
            //     Self::extract_expr_calls(&expr2.kind);
            // }
            // ExprKind::Unary(_, expr) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Cast(expr, _) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Type(expr, _) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::DropTemps(expr) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::If(expr1, expr2, Some(expr3)) => {
            //     Self::extract_expr_calls(&expr1.kind);
            //     Self::extract_expr_calls(&expr2.kind);
            //     Self::extract_expr_calls(&expr3.kind);
            // }
            // ExprKind::Loop(block, _, _, _) => {
            //     for stmt in block.stmts {
            //         if let StmtKind::Expr(expr) | StmtKind::Semi(expr) = &stmt.kind {
            //             Self::extract_expr_calls(&expr.kind);
            //         }
            //     }
            //     if let Some(ref expr) = block.expr {
            //         Self::extract_expr_calls(&expr.kind);
            //     }
            // }
            // ExprKind::Match(expr, _, _) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Block(block, _) => {
            //     for stmt in block.stmts {
            //         if let StmtKind::Expr(expr) | StmtKind::Semi(expr) = &stmt.kind {
            //             Self::extract_expr_calls(&expr.kind);
            //         }
            //     }
            //     if let Some(ref expr) = block.expr {
            //         Self::extract_expr_calls(&expr.kind);
            //     }
            // }
            // ExprKind::Assign(expr1, expr2, _) => {
            //     Self::extract_expr_calls(&expr1.kind);
            //     Self::extract_expr_calls(&expr2.kind);
            // }
            // ExprKind::AssignOp(_, expr1, expr2) => {
            //     Self::extract_expr_calls(&expr1.kind);
            //     Self::extract_expr_calls(&expr2.kind);
            // }
            // ExprKind::Field(expr, _) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Index(expr1, expr2, _) => {
            //     Self::extract_expr_calls(&expr1.kind);
            //     Self::extract_expr_calls(&expr2.kind);
            // }
            // ExprKind::AddrOf(_, _, expr) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Break(_, Some(expr)) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Ret(Some(expr)) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Become(expr) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Struct(_, _, Some(expr)) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Repeat(expr, _) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            // ExprKind::Yield(expr, _) => {
            //     Self::extract_expr_calls(&expr.kind);
            // }
            
            _ => {}
        }
    }
}

impl<'tcx> Visitor<'tcx> for ContainsUnsafe<'tcx> {
    //type Map = rustc_middle::hir::map::Map<'tcx>;
    type NestedFilter = nested_filter::OnlyBodies;

    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
        use rustc_hir::BlockCheckMode;
        if let BlockCheckMode::UnsafeBlock(_unsafe_source) = block.rules {
            self.block_unsafe = true;
        }
        intravisit::walk_block(self, block);
    }

    fn visit_expr(&mut self, ex: &'tcx Expr<'tcx>) {
        Self::extract_expr_calls(&ex.kind);
        if let ExprKind::MethodCall(_, _, _, _) = &ex.kind {

        }
        intravisit::walk_expr(self, ex);
    }
}

/// (`DefId` of ADT) => Vec<(HirId of relevant impl block, impl_self_ty)>
/// We use this map to quickly access associated impl blocks per ADT.
/// `impl_self_ty` in the return value may differ from `tcx.type_of(ADT.DefID)`,
/// as different instantiations of the same ADT are distinct `Ty`s.
/// (e.g. Foo<i32, i64>, Foo<String, i32>)
pub type AdtImplMap<'tcx> = FxHashMap<DefId, Vec<(LocalDefId, Ty<'tcx>)>>;

/// Create & initialize `AdtImplMap`.
/// `AdtImplMap` is initialized before analysis of each crate,
/// avoiding quadratic complexity of scanning all impl blocks for each ADT.
pub fn create_adt_impl_map<'tcx>(tcx: TyCtxt<'tcx>) -> AdtImplMap<'tcx> {
    let mut map = FxHashMap::default();

    for item_id in tcx.hir().items() {
        if let ItemKind::Impl(Impl { self_ty, .. }) = tcx.hir().item(item_id).kind {
            // `Self` type of the given impl block.
            let impl_self_ty = tcx.type_of(self_ty.hir_id.owner).skip_binder();

            if let ty::Adt(impl_self_adt_def, _impl_substs) = impl_self_ty.kind() {
                // We use `AdtDef.did` as key for `AdtImplMap`.
                // For any crazy instantiation of the same generic ADT (Foo<i32>, Foo<String>, etc..),
                // `AdtDef.did` refers to the original ADT definition.
                // Thus it can be used to map & collect impls for all instantitations of the same ADT.

                map.entry(impl_self_adt_def.did())
                    .or_insert_with(|| Vec::new())
                    .push((tcx.hir().item(item_id).owner_id.def_id, impl_self_ty));
            }
        }
    }
    map
}