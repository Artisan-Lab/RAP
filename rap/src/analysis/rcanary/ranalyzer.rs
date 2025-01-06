pub mod inter_visitor;
pub mod intra_visitor;
pub mod order;
pub mod ownership;

use rustc_middle::mir::{Body, Terminator};
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::DefId;

use super::{rCanary, IcxMut, IcxSliceMut, Rcx, RcxMut};
use crate::analysis::core::heap_item::{
    mir_body, type_visitor::TyWithIndex, AdtOwner, OwnershipLayout, Unique,
};
use crate::Elapsed;
use ownership::{IntraVar, Taint};

use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt::{Debug, Formatter};

pub type MirGraph = HashMap<DefId, Graph>;
pub type ToPo = Vec<usize>;
pub type Edges = Vec<Vec<usize>>;

#[derive(Debug, Clone)]
pub struct Graph {
    e: Edges,
    pre: Edges,
    topo: ToPo,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            e: Vec::default(),
            pre: Vec::default(),
            topo: Vec::default(),
        }
    }
}

impl Graph {
    pub fn new(len: usize) -> Self {
        Graph {
            e: vec![Vec::new(); len],
            pre: vec![Vec::new(); len],
            topo: Vec::new(),
        }
    }

    pub fn get_edges(&self) -> &Edges {
        &self.e
    }

    pub fn get_edges_mut(&mut self) -> &mut Edges {
        &mut self.e
    }

    pub fn get_pre(&self) -> &Edges {
        &self.pre
    }

    pub fn get_pre_mut(&mut self) -> &mut Edges {
        &mut self.pre
    }

    pub fn get_topo(&self) -> &ToPo {
        &self.topo
    }

    pub fn get_topo_mut(&mut self) -> &mut ToPo {
        &mut self.topo
    }
}

pub struct FlowAnalysis<'tcx, 'a> {
    rcx: &'a mut rCanary<'tcx>,
    fn_set: Unique,
}

impl<'tcx, 'a> FlowAnalysis<'tcx, 'a> {
    pub fn new(rcx: &'a mut rCanary<'tcx>) -> Self {
        Self {
            rcx,
            fn_set: HashSet::new(),
        }
    }

    pub fn fn_set(&self) -> &Unique {
        &self.fn_set
    }

    pub fn fn_set_mut(&mut self) -> &mut Unique {
        &mut self.fn_set
    }

    pub fn mir_graph(&self) -> &MirGraph {
        self.rcx().mir_graph()
    }

    pub fn mir_graph_mut(&mut self) -> &mut MirGraph {
        self.rcx_mut().mir_graph_mut()
    }

    pub fn start(&mut self) {
        // this phase determines the final order of all basic blocks for us to visit
        // Note: we will not visit the clean-up blocks (unwinding)
        self.order();
        // this phase will generate the Intra procedural visitor for us to visit the block
        // note that the inter procedural part is inside in this function but cod in module inter_visitor
        self.intra_run();

        // rap_info!("@@@@@@@@@@@@@Build Analysis:{:?}", self.rcx().get_time_build());
        // rap_info!("@@@@@@@@@@@@@Solve Analysis:{:?}", self.rcx().get_time_solve());
    }
}

impl<'tcx, 'o, 'a> RcxMut<'tcx, 'o, 'a> for FlowAnalysis<'tcx, 'a> {
    #[inline(always)]
    fn rcx(&'o self) -> &'o rCanary<'tcx> {
        self.rcx
    }

    #[inline(always)]
    fn rcx_mut(&'o mut self) -> &'o mut rCanary<'tcx> {
        &mut self.rcx
    }

    #[inline(always)]
    fn tcx(&'o self) -> TyCtxt<'tcx> {
        self.rcx().tcx()
    }
}

#[derive(Clone, Debug)]
pub struct NodeOrder<'tcx> {
    body: &'tcx Body<'tcx>,
    graph: Graph,
}

impl<'tcx> NodeOrder<'tcx> {
    pub fn new(body: &'tcx Body<'tcx>) -> Self {
        let len = body.basic_blocks.len();
        Self {
            body,
            graph: Graph::new(len),
        }
    }

    #[inline(always)]
    pub fn body(&self) -> &'tcx Body<'tcx> {
        self.body
    }

    #[inline(always)]
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    #[inline(always)]
    pub fn graph_mut(&mut self) -> &mut Graph {
        &mut self.graph
    }
}

struct IntraFlowAnalysis<'tcx, 'ctx, 'a> {
    rcx: &'a rCanary<'tcx>,
    icx: IntraFlowContext<'tcx, 'ctx>,
    icx_slice: IcxSliceFroBlock<'tcx, 'ctx>,
    did: DefId,
    body: &'a Body<'tcx>,
    graph: &'a Graph,
    elasped: Elapsed,
    taint_flag: bool,
    taint_source: Vec<Terminator<'tcx>>,
}

impl<'tcx, 'ctx, 'a> IntraFlowAnalysis<'tcx, 'ctx, 'a> {
    pub fn new(
        rcx: &'a rCanary<'tcx>,
        did: DefId,
        //unique: &'a mut Unique,
    ) -> Self {
        let body = mir_body(rcx.tcx(), did);
        let v_len = body.local_decls.len();
        let b_len = body.basic_blocks.len();
        let graph = rcx.mir_graph().get(&did).unwrap();

        Self {
            rcx,
            icx: IntraFlowContext::new(b_len, v_len),
            icx_slice: IcxSliceFroBlock::new_for_block_0(v_len),
            did,
            body,
            graph,
            elasped: (0, 0),
            taint_flag: false,
            taint_source: Vec::default(),
        }
    }

    #[allow(dead_code)]
    pub fn did(&self) -> DefId {
        self.did
    }

    pub fn body(&self) -> &'a Body<'tcx> {
        self.body
    }

    pub fn owner(&self) -> &AdtOwner {
        self.rcx().adt_owner()
    }

    pub fn graph(&self) -> &Graph {
        self.graph
    }

    pub fn get_time_build(&self) -> i64 {
        self.elasped.0
    }

    pub fn get_time_solve(&self) -> i64 {
        self.elasped.1
    }

    pub fn add_taint(&mut self, terminator: Terminator<'tcx>) {
        self.taint_source.push(terminator);
    }
}

impl<'tcx, 'ctx, 'o, 'a> Rcx<'tcx, 'o, 'a> for IntraFlowAnalysis<'tcx, 'ctx, 'a> {
    #[inline(always)]
    fn rcx(&'o self) -> &'a rCanary<'tcx> {
        self.rcx
    }

    #[inline(always)]
    fn tcx(&'o self) -> TyCtxt<'tcx> {
        self.rcx.tcx()
    }
}

impl<'tcx, 'ctx, 'o, 'a> IcxMut<'tcx, 'ctx, 'o> for IntraFlowAnalysis<'tcx, 'ctx, 'a> {
    #[inline(always)]
    fn icx(&'o self) -> &'o IntraFlowContext<'tcx, 'ctx> {
        &self.icx
    }

    #[inline(always)]
    fn icx_mut(&'o mut self) -> &'o mut IntraFlowContext<'tcx, 'ctx> {
        &mut self.icx
    }
}

impl<'tcx, 'ctx, 'o, 'a> IcxSliceMut<'tcx, 'ctx, 'o> for IntraFlowAnalysis<'tcx, 'ctx, 'a> {
    #[inline(always)]
    fn icx_slice(&'o self) -> &'o IcxSliceFroBlock<'tcx, 'ctx> {
        &self.icx_slice
    }

    #[inline(always)]
    fn icx_slice_mut(&'o mut self) -> &'o mut IcxSliceFroBlock<'tcx, 'ctx> {
        &mut self.icx_slice
    }
}

#[derive(Debug, Clone)]
pub struct IntraFlowContext<'tcx, 'ctx> {
    taint: IOPairForGraph<Taint<'tcx>>,
    var: IOPairForGraph<IntraVar<'ctx>>,
    len: IOPairForGraph<usize>,
    // the ty in icx is the Rust ownership layout of the pointing instance
    // Note: the ty is not the exact ty of the local
    ty: IOPairForGraph<TyWithIndex<'tcx>>,
    layout: IOPairForGraph<OwnershipLayout>,
}

impl<'tcx, 'ctx, 'icx> IntraFlowContext<'tcx, 'ctx> {
    pub fn new(b_len: usize, v_len: usize) -> Self {
        Self {
            taint: IOPairForGraph::new(b_len, v_len),
            var: IOPairForGraph::new(b_len, v_len),
            len: IOPairForGraph::new(b_len, v_len),
            ty: IOPairForGraph::new(b_len, v_len),
            layout: IOPairForGraph::new(b_len, v_len),
        }
    }

    pub fn taint(&self) -> &IOPairForGraph<Taint<'tcx>> {
        &self.taint
    }

    pub fn taint_mut(&mut self) -> &mut IOPairForGraph<Taint<'tcx>> {
        &mut self.taint
    }

    pub fn var(&self) -> &IOPairForGraph<IntraVar<'ctx>> {
        &self.var
    }

    pub fn var_mut(&mut self) -> &mut IOPairForGraph<IntraVar<'ctx>> {
        &mut self.var
    }

    pub fn len(&self) -> &IOPairForGraph<usize> {
        &self.len
    }

    pub fn len_mut(&mut self) -> &mut IOPairForGraph<usize> {
        &mut self.len
    }

    pub fn ty(&self) -> &IOPairForGraph<TyWithIndex<'tcx>> {
        &self.ty
    }

    pub fn ty_mut(&mut self) -> &mut IOPairForGraph<TyWithIndex<'tcx>> {
        &mut self.ty
    }

    pub fn layout(&self) -> &IOPairForGraph<OwnershipLayout> {
        &self.layout
    }

    pub fn layout_mut(&mut self) -> &mut IOPairForGraph<OwnershipLayout> {
        &mut self.layout
    }

    pub fn derive_from_pre_node(&mut self, from: usize, to: usize) {
        // derive the storage from the pre node
        *self.taint_mut().get_g_mut()[to].get_i_mut() =
            self.taint_mut().get_g_mut()[from].get_o_mut().clone();

        // derive the var vector from the pre node
        *self.var_mut().get_g_mut()[to].get_i_mut() =
            self.var_mut().get_g_mut()[from].get_o_mut().clone();

        // derive the len vector from the pre node
        *self.len_mut().get_g_mut()[to].get_i_mut() =
            self.len_mut().get_g_mut()[from].get_o_mut().clone();

        // derive the ty vector from the pre node
        *self.ty_mut().get_g_mut()[to].get_i_mut() =
            self.ty_mut().get_g_mut()[from].get_o_mut().clone();

        // derive the layout vector from the pre node
        *self.layout_mut().get_g_mut()[to].get_i_mut() =
            self.layout_mut().get_g_mut()[from].get_o_mut().clone();
    }

    pub fn derive_from_icx_slice(&mut self, from: IcxSliceFroBlock<'tcx, 'ctx>, to: usize) {
        *self.taint_mut().get_g_mut()[to].get_o_mut() = from.taint;

        *self.var_mut().get_g_mut()[to].get_o_mut() = from.var;

        *self.len_mut().get_g_mut()[to].get_o_mut() = from.len;

        *self.ty_mut().get_g_mut()[to].get_o_mut() = from.ty;

        *self.layout_mut().get_g_mut()[to].get_o_mut() = from.layout;
    }
}

#[derive(Debug, Clone, Default)]
pub struct InOutPair<T: Debug + Clone + Default> {
    i: Vec<T>,
    o: Vec<T>,
}

impl<T> InOutPair<T>
where
    T: Debug + Clone + Default,
{
    pub fn new(len: usize) -> Self {
        Self {
            i: vec![T::default(); len],
            o: vec![T::default(); len],
        }
    }

    pub fn get_i(&self) -> &Vec<T> {
        &self.i
    }

    pub fn get_o(&self) -> &Vec<T> {
        &self.o
    }

    pub fn get_i_mut(&mut self) -> &mut Vec<T> {
        &mut self.i
    }

    pub fn get_o_mut(&mut self) -> &mut Vec<T> {
        &mut self.o
    }

    pub fn len(&self) -> usize {
        self.i.len()
    }
}

#[derive(Debug, Clone, Default)]
pub struct IOPairForGraph<T: Debug + Clone + Default> {
    pair_graph: Vec<InOutPair<T>>,
}

impl<T> IOPairForGraph<T>
where
    T: Debug + Clone + Default,
{
    pub fn new(b_len: usize, v_len: usize) -> Self {
        Self {
            pair_graph: vec![InOutPair::new(v_len); b_len],
        }
    }

    pub fn get_g(&self) -> &Vec<InOutPair<T>> {
        &self.pair_graph
    }

    pub fn get_g_mut(&mut self) -> &mut Vec<InOutPair<T>> {
        &mut self.pair_graph
    }
}

#[derive(Clone, Default)]
pub struct IcxSliceFroBlock<'tcx, 'ctx> {
    taint: Vec<Taint<'tcx>>,
    var: Vec<IntraVar<'ctx>>,
    len: Vec<usize>,
    // the ty in icx is the Rust ownership layout of the pointing instance
    // Note: the ty is not the exact ty of the local
    ty: Vec<TyWithIndex<'tcx>>,
    layout: Vec<OwnershipLayout>,
}

impl<'tcx, 'ctx> IcxSliceFroBlock<'tcx, 'ctx> {
    pub fn new_in(icx: &mut IntraFlowContext<'tcx, 'ctx>, idx: usize) -> Self {
        Self {
            taint: icx.taint_mut().get_g_mut()[idx].get_i_mut().clone(),
            var: icx.var_mut().get_g_mut()[idx].get_i_mut().clone(),
            len: icx.len_mut().get_g_mut()[idx].get_i_mut().clone(),
            ty: icx.ty_mut().get_g_mut()[idx].get_i_mut().clone(),
            layout: icx.layout_mut().get_g_mut()[idx].get_i_mut().clone(),
        }
    }

    pub fn new_out(icx: &mut IntraFlowContext<'tcx, 'ctx>, idx: usize) -> Self {
        Self {
            taint: icx.taint_mut().get_g_mut()[idx].get_o_mut().clone(),
            var: icx.var_mut().get_g_mut()[idx].get_o_mut().clone(),
            len: icx.len_mut().get_g_mut()[idx].get_o_mut().clone(),
            ty: icx.ty_mut().get_g_mut()[idx].get_o_mut().clone(),
            layout: icx.layout_mut().get_g_mut()[idx].get_o_mut().clone(),
        }
    }

    pub fn new_for_block_0(len: usize) -> Self {
        Self {
            taint: vec![Taint::default(); len],
            var: vec![IntraVar::default(); len],
            len: vec![0; len],
            ty: vec![TyWithIndex::default(); len],
            layout: vec![Vec::new(); len],
        }
    }

    pub fn taint(&self) -> &Vec<Taint<'tcx>> {
        &self.taint
    }

    pub fn taint_mut(&mut self) -> &mut Vec<Taint<'tcx>> {
        &mut self.taint
    }

    pub fn var(&self) -> &Vec<IntraVar<'ctx>> {
        &self.var
    }

    pub fn var_mut(&mut self) -> &mut Vec<IntraVar<'ctx>> {
        &mut self.var
    }

    pub fn len(&self) -> &Vec<usize> {
        &self.len
    }

    pub fn len_mut(&mut self) -> &mut Vec<usize> {
        &mut self.len
    }

    pub fn ty(&self) -> &Vec<TyWithIndex<'tcx>> {
        &self.ty
    }

    pub fn ty_mut(&mut self) -> &mut Vec<TyWithIndex<'tcx>> {
        &mut self.ty
    }

    pub fn layout(&self) -> &Vec<OwnershipLayout> {
        &self.layout
    }

    pub fn layout_mut(&mut self) -> &mut Vec<OwnershipLayout> {
        &mut self.layout
    }

    pub fn taint_merge(&mut self, another: &IcxSliceFroBlock<'tcx, 'ctx>, u: usize) {
        if another.taint()[u].is_untainted() {
            return;
        }

        if self.taint()[u].is_untainted() {
            self.taint_mut()[u] = another.taint()[u].clone();
        } else {
            for elem in another.taint()[u].set().clone() {
                self.taint_mut()[u].insert(elem);
            }
        }
    }
}

impl<'tcx, 'ctx> Debug for IcxSliceFroBlock<'tcx, 'ctx> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "IcxSliceForBlock\n     {:?}\n     {:?}\n     {:?}\n     {:?}\n     {:?}",
            self.taint(),
            self.len(),
            self.var(),
            self.layout(),
            self.ty(),
        )
    }
}

#[derive(Debug, Copy, Clone, Hash)]
pub enum Z3GoalDisplay {
    Verbose,
    Disabled,
}

pub fn is_z3_goal_verbose() -> bool {
    match env::var_os("Z3") {
        Some(_) => true,
        _ => false,
    }
}

#[derive(Debug, Copy, Clone, Hash)]
pub enum IcxSliceDisplay {
    Verbose,
    Disabled,
}

pub fn is_icx_slice_verbose() -> bool {
    match env::var_os("ICX_SLICE") {
        Some(_) => true,
        _ => false,
    }
}
