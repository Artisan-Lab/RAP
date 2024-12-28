use super::types::*;
use crate::analysis::core::alias::FnRetAlias;
use crate::rap_debug;
use crate::utils::source::*;
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_middle::mir::{
    BasicBlock, Const, Operand, Place, Rvalue, StatementKind, Terminator, TerminatorKind,
    UnwindAction,
};
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::DefId;
use rustc_span::Span;
use std::cmp::min;
use std::vec::Vec;

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum AssignType {
    Copy,
    Move,
    InitBox,
    Variant,
}

//self-defined assignments structure.
#[derive(Debug, Clone)]
pub struct Assignment<'tcx> {
    pub lv: Place<'tcx>,
    pub rv: Place<'tcx>,
    pub atype: AssignType,
    pub span: Span,
}

impl<'tcx> Assignment<'tcx> {
    pub fn new(
        lv: Place<'tcx>,
        rv: Place<'tcx>,
        atype: AssignType,
        span: Span,
    ) -> Assignment<'tcx> {
        Assignment {
            lv,
            rv,
            atype,
            span,
        }
    }
}

/*
 * Self-defined basicblock structure;
 * Used both for the original CFG and after SCC.
 */

#[derive(Debug, Clone)]
pub struct BlockNode<'tcx> {
    pub index: usize,
    pub next: FxHashSet<usize>,
    pub assignments: Vec<Assignment<'tcx>>,
    pub calls: Vec<Terminator<'tcx>>,
    //store the index of the basic blocks as a SCC node.
    pub scc_sub_blocks: Vec<usize>,
    //store const values defined in this block, i.e., which id has what value;
    pub const_value: Vec<(usize, usize)>,
    //store switch stmts in current block for the path filtering in path-sensitive analysis.
    pub switch_stmts: Vec<Terminator<'tcx>>,
}

impl<'tcx> BlockNode<'tcx> {
    pub fn new(index: usize) -> BlockNode<'tcx> {
        BlockNode {
            index,
            next: FxHashSet::<usize>::default(),
            assignments: Vec::<Assignment<'tcx>>::new(),
            calls: Vec::<Terminator<'tcx>>::new(),
            scc_sub_blocks: Vec::<usize>::new(),
            const_value: Vec::<(usize, usize)>::new(),
            switch_stmts: Vec::<Terminator<'tcx>>::new(),
        }
    }

    pub fn add_next(&mut self, index: usize) {
        self.next.insert(index);
    }
}

#[derive(Debug, Clone)]
pub struct ValueNode {
    pub index: usize, // node index
    pub local: usize, // location?
    pub need_drop: bool,
    pub may_drop: bool,
    pub kind: TyKind,
    pub father: usize,
    pub field_id: usize, // the field id of its father node.
    pub alias: Vec<usize>,
    pub fields: FxHashMap<usize, usize>,
}

impl ValueNode {
    pub fn new(index: usize, local: usize, need_drop: bool, may_drop: bool) -> Self {
        ValueNode {
            index,
            local,
            need_drop,
            father: local,
            field_id: usize::MAX,
            alias: vec![index],
            may_drop,
            kind: TyKind::Adt,
            fields: FxHashMap::default(),
        }
    }

    pub fn is_tuple(&self) -> bool {
        self.kind == TyKind::Tuple
    }

    pub fn is_ptr(&self) -> bool {
        self.kind == TyKind::RawPtr || self.kind == TyKind::Ref
    }

    pub fn is_ref(&self) -> bool {
        self.kind == TyKind::Ref
    }

    pub fn is_corner_case(&self) -> bool {
        self.kind == TyKind::CornerCase
    }
}

pub struct MopGraph<'tcx> {
    pub def_id: DefId,
    pub tcx: TyCtxt<'tcx>,
    pub span: Span,
    // contains all varibles (including fields) as values.
    pub values: Vec<ValueNode>,
    // contains all blocks in the CFG
    pub blocks: Vec<BlockNode<'tcx>>,
    pub arg_size: usize,
    // we shrink a SCC into a node and use a scc node to represent the SCC.
    pub scc_indices: Vec<usize>,
    // record the constant value during safedrop checking, i.e., which id has what value.
    pub constant: FxHashMap<usize, usize>,
    // contains the return results for inter-procedure analysis.
    pub ret_alias: FnRetAlias,
    // a threhold to avoid path explosion.
    pub visit_times: usize,
}

impl<'tcx> MopGraph<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, def_id: DefId) -> MopGraph<'tcx> {
        let fn_name = get_fn_name(tcx, def_id);
        rap_debug!("Building a new MoP graph for: {:?}", fn_name);
        // handle variables
        let body = tcx.optimized_mir(def_id);
        let locals = &body.local_decls;
        let arg_size = body.arg_count;
        let mut values = Vec::<ValueNode>::new();
        let param_env = tcx.param_env(def_id);
        for (local, local_decl) in locals.iter_enumerated() {
            let need_drop = local_decl.ty.needs_drop(tcx, param_env); // the type is drop
            let may_drop = !is_not_drop(tcx, local_decl.ty);
            let mut node = ValueNode::new(
                local.as_usize(),
                local.as_usize(),
                need_drop,
                need_drop || may_drop,
            );
            node.kind = kind(local_decl.ty);
            values.push(node);
        }

        let basicblocks = &body.basic_blocks;
        let mut blocks = Vec::<BlockNode<'tcx>>::new();
        let mut scc_indices = Vec::<usize>::new();

        // handle each basicblock
        for i in 0..basicblocks.len() {
            scc_indices.push(i);
            let iter = BasicBlock::from(i);
            let terminator = basicblocks[iter].terminator.clone().unwrap();
            let mut cur_bb = BlockNode::new(i);

            // handle general statements
            for stmt in &basicblocks[iter].statements {
                /* Assign is a tuple defined as Assign(Box<(Place<'tcx>, Rvalue<'tcx>)>) */
                let span = stmt.source_info.span;
                if let StatementKind::Assign(ref assign) = stmt.kind {
                    let lv_local = assign.0.local.as_usize(); // assign.0 is a Place
                    let lv = assign.0;
                    match assign.1 {
                        // assign.1 is a Rvalue
                        Rvalue::Use(ref x) => {
                            match x {
                                Operand::Copy(ref p) => {
                                    let rv_local = p.local.as_usize();
                                    if values[lv_local].may_drop && values[rv_local].may_drop {
                                        let rv = *p;
                                        let assign =
                                            Assignment::new(lv, rv, AssignType::Copy, span);
                                        cur_bb.assignments.push(assign);
                                    }
                                }
                                Operand::Move(ref p) => {
                                    let rv_local = p.local.as_usize();
                                    if values[lv_local].may_drop && values[rv_local].may_drop {
                                        let rv = *p;
                                        let assign =
                                            Assignment::new(lv, rv, AssignType::Move, span);
                                        cur_bb.assignments.push(assign);
                                    }
                                }
                                Operand::Constant(ref constant) => {
                                    /* We should check the correctness due to the update of rustc */
                                    match constant.const_ {
                                        Const::Ty(_ty, const_value) => {
                                            if let Some((_ty, scalar)) =
                                                const_value.try_eval_scalar_int(tcx, param_env)
                                            {
                                                let val = scalar.to_uint(scalar.size());
                                                cur_bb.const_value.push((lv_local, val as usize));
                                            }
                                        }
                                        Const::Unevaluated(_const_value, _ty) => {}
                                        Const::Val(const_value, _ty) => {
                                            if let Some(scalar) = const_value.try_to_scalar_int() {
                                                let val = scalar.to_uint(scalar.size());
                                                cur_bb.const_value.push((lv_local, val as usize));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Rvalue::Ref(_, _, ref p) | Rvalue::RawPtr(_, ref p) => {
                            let rv_local = p.local.as_usize();
                            if values[lv_local].may_drop && values[rv_local].may_drop {
                                let rv = *p;
                                let assign = Assignment::new(lv, rv, AssignType::Copy, span);
                                cur_bb.assignments.push(assign);
                            }
                        }
                        Rvalue::ShallowInitBox(ref x, _) => {
                            /*
                             * Original ShllowInitBox is a two-level pointer: lvl0 -> lvl1 -> lvl2
                             * Since our alias analysis does not consider multi-level pointer,
                             * We simplify it as: lvl0
                             */
                            if !values[lv_local].fields.contains_key(&0) {
                                let mut lvl0 = ValueNode::new(values.len(), lv_local, false, true);
                                lvl0.field_id = 0;
                                values[lv_local].fields.insert(0, lvl0.index);
                                values.push(lvl0);
                            }
                            match x {
                                Operand::Copy(ref p) | Operand::Move(ref p) => {
                                    let rv_local = p.local.as_usize();
                                    if values[lv_local].may_drop && values[rv_local].may_drop {
                                        let rv = *p;
                                        let assign =
                                            Assignment::new(lv, rv, AssignType::InitBox, span);
                                        cur_bb.assignments.push(assign);
                                    }
                                }
                                Operand::Constant(_) => {}
                            }
                        }
                        Rvalue::Cast(_, ref x, _) => match x {
                            Operand::Copy(ref p) => {
                                let rv_local = p.local.as_usize();
                                if values[lv_local].may_drop && values[rv_local].may_drop {
                                    let rv = *p;
                                    let assign = Assignment::new(lv, rv, AssignType::Copy, span);
                                    cur_bb.assignments.push(assign);
                                }
                            }
                            Operand::Move(ref p) => {
                                let rv_local = p.local.as_usize();
                                if values[lv_local].may_drop && values[rv_local].may_drop {
                                    let rv = *p;
                                    let assign = Assignment::new(lv, rv, AssignType::Move, span);
                                    cur_bb.assignments.push(assign);
                                }
                            }
                            Operand::Constant(_) => {}
                        },
                        Rvalue::Aggregate(_, ref x) => {
                            for each_x in x {
                                match each_x {
                                    Operand::Copy(ref p) | Operand::Move(ref p) => {
                                        let rv_local = p.local.as_usize();
                                        if values[lv_local].may_drop && values[rv_local].may_drop {
                                            let rv = *p;
                                            let assign =
                                                Assignment::new(lv, rv, AssignType::Copy, span);
                                            cur_bb.assignments.push(assign);
                                        }
                                    }
                                    Operand::Constant(_) => {}
                                }
                            }
                        }
                        Rvalue::Discriminant(ref p) => {
                            let rv = *p;
                            let assign = Assignment::new(lv, rv, AssignType::Variant, span);
                            cur_bb.assignments.push(assign);
                        }
                        _ => {}
                    }
                }
            }

            // handle terminator statements
            match terminator.kind {
                TerminatorKind::Goto { ref target } => {
                    cur_bb.add_next(target.as_usize());
                }
                TerminatorKind::SwitchInt {
                    discr: _,
                    ref targets,
                } => {
                    cur_bb.switch_stmts.push(terminator.clone());
                    for (_, ref target) in targets.iter() {
                        cur_bb.add_next(target.as_usize());
                    }
                    cur_bb.add_next(targets.otherwise().as_usize());
                }
                TerminatorKind::UnwindResume
                | TerminatorKind::Return
                | TerminatorKind::UnwindTerminate(_)
                | TerminatorKind::CoroutineDrop
                | TerminatorKind::Unreachable => {}
                TerminatorKind::Drop {
                    place: _,
                    ref target,
                    ref unwind,
                    replace: _,
                } => {
                    cur_bb.add_next(target.as_usize());
                    if let UnwindAction::Cleanup(target) = unwind {
                        cur_bb.add_next(target.as_usize());
                    }
                }
                TerminatorKind::Call {
                    func: _,
                    args: _,
                    destination: _,
                    ref target,
                    ref unwind,
                    call_source: _,
                    fn_span: _,
                } => {
                    if let Some(tt) = target {
                        cur_bb.add_next(tt.as_usize());
                    }
                    if let UnwindAction::Cleanup(tt) = unwind {
                        cur_bb.add_next(tt.as_usize());
                    }
                    cur_bb.calls.push(terminator.clone());
                }
                TerminatorKind::TailCall { .. } => todo!(),
                TerminatorKind::Assert {
                    cond: _,
                    expected: _,
                    msg: _,
                    ref target,
                    ref unwind,
                } => {
                    cur_bb.add_next(target.as_usize());
                    if let UnwindAction::Cleanup(target) = unwind {
                        cur_bb.add_next(target.as_usize());
                    }
                }
                TerminatorKind::Yield {
                    value: _,
                    ref resume,
                    resume_arg: _,
                    ref drop,
                } => {
                    cur_bb.add_next(resume.as_usize());
                    if let Some(target) = drop {
                        cur_bb.add_next(target.as_usize());
                    }
                }
                TerminatorKind::FalseEdge {
                    ref real_target,
                    imaginary_target: _,
                } => {
                    cur_bb.add_next(real_target.as_usize());
                }
                TerminatorKind::FalseUnwind {
                    ref real_target,
                    unwind: _,
                } => {
                    cur_bb.add_next(real_target.as_usize());
                }
                TerminatorKind::InlineAsm {
                    template: _,
                    operands: _,
                    options: _,
                    line_spans: _,
                    ref unwind,
                    targets,
                    asm_macro: _,
                } => {
                    for target in targets {
                        cur_bb.add_next(target.as_usize());
                    }
                    if let UnwindAction::Cleanup(target) = unwind {
                        cur_bb.add_next(target.as_usize());
                    }
                }
            }
            blocks.push(cur_bb);
        }

        MopGraph {
            def_id,
            tcx,
            span: body.span,
            blocks,
            values,
            arg_size,
            scc_indices,
            constant: FxHashMap::default(),
            ret_alias: FnRetAlias::new(arg_size),
            visit_times: 0,
        }
    }

    pub fn tarjan(
        &mut self,
        index: usize,
        stack: &mut Vec<usize>,
        instack: &mut FxHashSet<usize>,
        dfn: &mut Vec<usize>,
        low: &mut Vec<usize>,
        time: &mut usize,
    ) {
        dfn[index] = *time;
        low[index] = *time;
        *time += 1;
        instack.insert(index);
        stack.push(index);
        let out_set = self.blocks[index].next.clone();
        for target in out_set {
            if dfn[target] == 0 {
                self.tarjan(target, stack, instack, dfn, low, time);
                low[index] = min(low[index], low[target]);
            } else if instack.contains(&target) {
                low[index] = min(low[index], dfn[target]);
            }
        }
        // generate SCC
        if dfn[index] == low[index] {
            loop {
                let node = stack.pop().unwrap();
                self.scc_indices[node] = index;
                instack.remove(&node);
                if index == node {
                    // we have found all nodes of the current scc.
                    break;
                }
                self.blocks[index].scc_sub_blocks.push(node);
                let nexts = self.blocks[node].next.clone();
                for i in nexts {
                    self.blocks[index].next.insert(i);
                }
            }
            /* remove next nodes which are already in the current SCC */
            let mut to_remove = Vec::new();
            for i in self.blocks[index].next.iter() {
                if self.scc_indices[*i] == index {
                    to_remove.push(*i);
                }
            }
            for i in to_remove {
                self.blocks[index].next.remove(&i);
            }
            /* To ensure a resonable order of blocks within one SCC,
             * so that the scc can be directly used for followup analysis without referencing the
             * original graph.
             * */
            self.blocks[index].scc_sub_blocks.reverse();
        }
    }

    // handle SCC
    pub fn solve_scc(&mut self) {
        let mut stack = Vec::<usize>::new();
        let mut instack = FxHashSet::<usize>::default();
        let mut dfn = vec![0_usize; self.blocks.len()];
        let mut low = vec![0_usize; self.blocks.len()];
        let mut time = 0;
        self.tarjan(0, &mut stack, &mut instack, &mut dfn, &mut low, &mut time);
    }
}
