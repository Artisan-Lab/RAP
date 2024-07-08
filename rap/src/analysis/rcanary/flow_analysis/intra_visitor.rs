use rustc_middle::ty::{self, Ty, TyKind, TypeVisitable};
use rustc_middle::mir::{Body, BasicBlock, BasicBlockData, Statement, StatementKind,
                        Terminator, Place, Rvalue, Local, Operand, ProjectionElem, TerminatorKind};
use rustc_target::abi::VariantIdx;

use crate::{rap_error, rap_warn};
use crate::analysis::rcanary::{Rcx, RcxMut, IcxMut, IcxSliceMut};
use crate::analysis::rcanary::type_analysis::{DefaultOwnership, mir_body, OwnershipLayout, RustBV,
                                              Unique, ownership::{OwnershipLayoutResult, RawTypeOwner},
                                              type_visitor::TyWithIndex};
use crate::analysis::rcanary::flow_analysis::{IntraFlowAnalysis, FlowAnalysis, IcxSliceFroBlock,
                                              is_icx_slice_verbose, ownership::IntraVar};

use z3::ast::{self, Ast};

use std::ops::Add;
use stopwatch::Stopwatch;
use colorful::{Color, Colorful};

use super::is_z3_goal_verbose;
// Fixme: arg.0
// Fixme: arg enum

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum AsgnKind {
    Assign,
    Reference,
    Pointer,
    Cast,
}

impl<'tcx, 'a> FlowAnalysis<'tcx, 'a>{
    pub fn intra_run(&mut self) {
        let tcx = self.tcx();
        let mir_keys = tcx.mir_keys(());
        let mut unique = Unique::new();

        for each_mir in mir_keys {

            let mut sw = Stopwatch::start_new();

            let def_id = each_mir.to_def_id();
            let body = mir_body(tcx, def_id);
            // for loop fee function analysis
            if body.basic_blocks.is_cfg_cyclic() { continue; }

            let mut cfg = z3::Config::new();
            cfg.set_model_generation(true);
            cfg.set_timeout_msec(1000);
            let ctx = z3::Context::new(&cfg);
            let goal = z3::Goal::new(&ctx, true, false, false);
            let solver = z3::Solver::new(&ctx);

            let mut intra_visitor = IntraFlowAnalysis::new(self.rcx, def_id, &mut unique);
            intra_visitor.visit_body(&ctx, &goal, &solver, body, &sw);

            let sec_build = intra_visitor.get_time_build();
            let sec_solve = intra_visitor.get_time_solve();

            self.rcx_mut().add_time_build(sec_build);
            self.rcx_mut().add_time_solve(sec_solve);

        }
    }
}

impl<'tcx, 'ctx, 'a> IntraFlowAnalysis<'tcx, 'ctx, 'a> {

    pub(crate) fn visit_body(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        body: &'tcx Body<'tcx>,
        sw: &Stopwatch,
    ) {
        let topo:Vec<usize> = self.graph().get_topo().iter().map(|id| *id).collect();
        for bidx in topo
        {
            let data = &body.basic_blocks[BasicBlock::from(bidx)];
            self.visit_block_data(ctx, goal, solver, data, sw, bidx);
        }

    }

    pub(crate) fn visit_block_data(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        data: &'tcx BasicBlockData<'tcx>,
        sw: &Stopwatch,
        bidx: usize,
    ) {
        self.preprocess_for_basic_block(ctx, goal, solver, sw, bidx);

        for (sidx, stmt) in data.statements.iter().enumerate() {
            self.visit_statement(ctx, goal, solver, data, stmt, bidx, sidx);
        }

        self.visit_terminator(ctx, goal, solver, data.terminator(), sw, bidx);

        self.reprocess_for_basic_block(bidx);

    }

    pub(crate) fn preprocess_for_basic_block(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        sw: &Stopwatch,
        bidx: usize
    ) {

        // For node 0 there is no pre node existed!
        if bidx == 0 {
            let mut icx_slice = IcxSliceFroBlock::new_for_block_0(self.body().local_decls.len());

            for arg_idx in 0..self.body().arg_count {
                let idx = arg_idx + 1;
                let ty = self.body().local_decls[Local::from_usize(idx)].ty;

                let ty_with_index = TyWithIndex::new(ty, None);
                if ty_with_index == TyWithIndex(None) {
                    self.handle_intra_var_unsupported(idx);
                    continue;
                }

                let default_layout = self.extract_default_ty_layout(ty, None);
                if !default_layout.is_owned() {
                    icx_slice.len_mut()[idx] = 0;
                    icx_slice.var_mut()[idx] = IntraVar::Unsupported;
                    icx_slice.ty_mut()[idx] = TyWithIndex(None);
                    continue;
                }
                let int = rustbv_to_int(&ownership_layout_to_rustbv(default_layout.layout()));

                let name = new_local_name(idx, 0, 0).add("_arg_init");
                let len = default_layout.layout().len();

                let new_bv = ast::BV::new_const(ctx, name, len as u32);
                let init_const = ast::BV::from_u64(ctx, int, len as u32);

                let constraint_init_arg = new_bv._eq(&init_const);

                goal.assert(&constraint_init_arg);
                solver.assert(&constraint_init_arg);

                icx_slice.len_mut()[idx] = len;
                icx_slice.var_mut()[idx] = IntraVar::Init(new_bv);
                icx_slice.ty_mut()[idx] = ty_with_index;
            }

            *self.icx_slice_mut() = icx_slice.clone();

            return;
        }

        let pre = &self.graph.pre[bidx];

        if pre.len() > 1 {
            // collect all pre nodes and generate their icx slice into a vector
            let mut v_pre_collect:Vec<IcxSliceFroBlock> = Vec::default();
            for idx in pre {
                v_pre_collect.push(IcxSliceFroBlock::new_out(self.icx_mut(), *idx));
            }

            // the result icx slice for updating the icx
            let mut ans_icx_slice = v_pre_collect[0].clone();
            let var_len = v_pre_collect[0].len().len();

            // for all variables
            for var_idx in 0..var_len {

                // the bv and len is using to generate new constrain
                // the ty is to check the consistency among the branches
                let mut using_for_and_bv:Option<ast::BV> = None;
                let mut ty = TyWithIndex::default();
                let mut len = 0;

                let mut unsupported = false;
                // for one variable in all pre basic blocks
                for idx in 0..v_pre_collect.len() {
                    // merge: ty = ty, len = len
                    let var = &v_pre_collect[idx].var()[var_idx];
                    if var.is_declared() {
                        continue;
                    }
                    if var.is_unsupported() {
                        unsupported = true;
                        ans_icx_slice.len_mut()[var_idx] = 0;
                        ans_icx_slice.var_mut()[var_idx] = IntraVar::Unsupported;
                        break;
                    }

                    // for now the len must not be zero and the var must not be decl/un..
                    let var_bv = var.extract();
                    if ty == TyWithIndex(None) {
                        ty = v_pre_collect[idx].ty()[var_idx].clone();
                        len = v_pre_collect[idx].len()[var_idx];

                        ans_icx_slice.ty_mut()[var_idx] = ty.clone();
                        ans_icx_slice.len_mut()[var_idx] = len;

                        using_for_and_bv = Some(var_bv.clone());
                    }

                    if ty != v_pre_collect[idx].ty()[var_idx] {
                        unsupported = true;
                        ans_icx_slice.len_mut()[var_idx] = 0;
                        ans_icx_slice.var_mut()[var_idx] = IntraVar::Unsupported;
                        break;
                    }

                    // use bv and to generate new bv
                    let bv_and = using_for_and_bv.unwrap().bvand(&var_bv);
                    using_for_and_bv = Some(bv_and);
                    ans_icx_slice.taint_merge(&v_pre_collect[idx], var_idx);
                }

                if unsupported || using_for_and_bv.is_none() {
                    *self.icx_slice_mut() = ans_icx_slice.clone();
                    continue;
                }

                let name = new_local_name(var_idx, bidx, 0).add("_phi");
                let phi_bv = ast::BV::new_const(ctx, name, len as u32);
                let constraint_phi = phi_bv._eq(&using_for_and_bv.unwrap());

                goal.assert(&constraint_phi);
                solver.assert(&constraint_phi);

                ans_icx_slice.var_mut()[var_idx] = IntraVar::Init(phi_bv);

                *self.icx_slice_mut() = ans_icx_slice.clone();
            }
        } else {
            if pre.len() == 0 { rap_error!("The pre node is empty, check the logic is safe to launch."); }
            self.icx_mut().derive_from_pre_node(pre[0], bidx);
            self.icx_slice = IcxSliceFroBlock::new_in(self.icx_mut(), bidx);
        }

        // println!("{:?} in {}", self.icx_slice(), bidx);
    }

    pub(crate) fn reprocess_for_basic_block(
        &mut self,
        bidx: usize
    ) {
        let icx_slice = self.icx_slice().clone();
        self.icx_slice = IcxSliceFroBlock::default();
        self.icx_mut().derive_from_icx_slice(icx_slice, bidx);
    }

    pub(crate) fn visit_statement(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        data: &'tcx BasicBlockData<'tcx>,
        stmt: &Statement<'tcx>,
        bidx: usize,
        sidx: usize,
    ) {

        match &stmt.kind {
            StatementKind::Assign(
                box(ref place, ref rvalue)
            ) => {
                help_debug_goal_stmt(ctx, goal, bidx, sidx);

                let l_local = place.local;
                let l_local_ty = self.body().local_decls[l_local].ty;

                let mut disc:Disc = None;

                // if l_local_ty.is_enum() {
                //     let stmt_disc = sidx + 1;
                //     if stmt_disc < data.statements.len() {
                //         match &data.statements[stmt_disc].kind {
                //             StatementKind::SetDiscriminant { place: disc_place, variant_index: vidx, }
                //             => {
                //                 let disc_local = disc_place.local;
                //                 if disc_local == l_local {
                //                     match extract_projection(disc_place) {
                //                         Some(prj) => {
                //                             if prj.is_unsupported() {
                //                                 self.handle_Intra_var_unsupported(l_local.as_usize());
                //                                 return;
                //                             }
                //                             disc = Some(*vidx);
                //                         },
                //                         None => (),
                //                     }
                //                 }
                //             },
                //             _ => (),
                //         }
                //     };
                // }

                self.visit_assign(ctx, goal, solver, place, rvalue, disc, bidx, sidx);

                if is_icx_slice_verbose() {
                    println!("IcxSlice in Assign: {} {}: {:?}\n{:?}\n", bidx, sidx, stmt.kind, self.icx_slice());
                }
            },
            StatementKind::StorageLive(local) => {
                self.handle_stmt_live(local, bidx);
            },
            StatementKind::StorageDead(local) => {
                self.handle_stmt_dead(local, bidx);
            },
            _ => (),
        }
    }

    pub(crate) fn visit_terminator(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        term: &'tcx Terminator<'tcx>,
        sw: &Stopwatch,
        bidx: usize,
    ){

        help_debug_goal_term(ctx, goal, bidx);

        match &term.kind {
            TerminatorKind::Drop { place, .. } => {
                self.handle_drop(ctx, goal, solver, place, bidx, false);
            },
            TerminatorKind::Call { func, args, destination, .. } => {
                self.handle_call(ctx, goal, solver, term.clone(), &func, &args, &destination, bidx);
            },
            TerminatorKind::Return => {
                self.handle_return(ctx, goal, solver, sw, bidx);
            }
            _ => (),
        }

        if is_icx_slice_verbose() {
            println!("IcxSlice in Terminator: {}: {:?}\n{:?}\n", bidx, term.kind, self.icx_slice());
        }
    }


    pub(crate) fn visit_assign(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        lplace: &Place<'tcx>,
        rvalue: &Rvalue<'tcx>,
        mut disc: Disc,
        bidx: usize,
        sidx: usize
    ) {
        let lvalue_has_projection  = has_projection(lplace);

        match rvalue {
            Rvalue::Use(op) => {
                let kind = AsgnKind::Assign;
                match op {
                    Operand::Copy(rplace) => {
                        let rvalue_has_projection = has_projection(rplace);
                        match (lvalue_has_projection, rvalue_has_projection) {
                            (true, true) => { self.handle_copy_field_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (true, false) => { self.handle_copy_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (false, true) => { self.handle_copy_from_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (false, false) => { self.handle_copy(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                        }
                    },
                    Operand::Move(rplace) => {
                        let rvalue_has_projection = has_projection(rplace);
                        match (lvalue_has_projection, rvalue_has_projection) {
                            (true, true) => { self.handle_move_field_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (true, false) => { self.handle_move_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (false, true) => { self.handle_move_from_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (false, false) => { self.handle_move(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                        }
                    },
                    _ => (),
                }
            },
            Rvalue::Ref(.., rplace) => {
                let kind = AsgnKind::Reference;
                let rvalue_has_projection = has_projection(rplace);
                match (lvalue_has_projection, rvalue_has_projection) {
                    (true, true) => { self.handle_copy_field_to_field(ctx, goal, solver, kind, lplace, rplace,disc,  bidx, sidx); },
                    (true, false) => { self.handle_copy_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                    (false, true) => { self.handle_copy_from_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                    (false, false) => { self.handle_copy(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                }
            },
            Rvalue::AddressOf(.., rplace) => {
                let kind = AsgnKind::Reference;
                let rvalue_has_projection = has_projection(rplace);
                match (lvalue_has_projection, rvalue_has_projection) {
                    (true, true) => { self.handle_copy_field_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                    (true, false) => { self.handle_copy_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                    (false, true) => { self.handle_copy_from_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                    (false, false) => { self.handle_copy(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                }
            },
            Rvalue::Cast(cast_kind, op, ..) => {
                let kind = AsgnKind::Cast;
                match op {
                    Operand::Copy(rplace) => {
                        let rvalue_has_projection = has_projection(rplace);
                        match (lvalue_has_projection, rvalue_has_projection) {
                            (true, true) => { self.handle_copy_field_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (true, false) => { self.handle_copy_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (false, true) => { self.handle_copy_from_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (false, false) => { self.handle_copy(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                        }
                    },
                    Operand::Move(rplace) => {
                        let rvalue_has_projection = has_projection(rplace);
                        match (lvalue_has_projection, rvalue_has_projection) {
                            (true, true) => { self.handle_move_field_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (true, false) => { self.handle_move_to_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (false, true) => { self.handle_move_from_field(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                            (false, false) => { self.handle_move(ctx, goal, solver, kind, lplace, rplace, disc, bidx, sidx); },
                        }
                    },
                    _ => (),
                }

            },
            _ => (),
        }
    }

    pub(crate) fn handle_copy(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        _kind: AsgnKind,
        lplace: &Place<'tcx>,
        rplace: &Place<'tcx>,
        mut disc: Disc,
        bidx: usize,
        sidx: usize
    ) {
        let llocal = lplace.local;
        let rlocal = rplace.local;

        let lu:usize = llocal.as_usize();
        let ru:usize = rlocal.as_usize();

        // if any rvalue or lplace is unsupported, then make them all unsupported and exit
        if self.icx_slice().var()[lu].is_unsupported() || self.icx_slice.var()[ru].is_unsupported() {
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }
        if !self.icx_slice().var[ru].is_init() {
            return;
        }

        // if the current layout of rvalue is 0, avoid the following analysis
        // e.g., a = b, b:[]
        if self.icx_slice().len()[ru] == 0 {
            // the len is 0 and ty is None which do not need update
            return;
        }

        // get the length of current variable to generate bit vector in the future
        let mut llen = self.icx_slice().len()[lu];
        let rlen = self.icx_slice().len()[ru];

        // extract the original z3 ast of the variable needed to prepare generating new
        let l_ori_bv :ast::BV;
        let r_ori_bv = self.icx_slice_mut().var_mut()[ru].extract();

        let mut is_ctor = true;
        if self.icx_slice().var()[lu].is_init() {
            // if the lvalue is not initialized for the first time (already initialized)
            // the constraint that promise the original value of lvalue that does not hold the ownership
            // e.g., y=x ,that y is non-owning => l=0
            // check the pointee layout (of) is same
            if self.icx_slice().ty()[lu] != self.icx_slice().ty[ru] {
                self.handle_intra_var_unsupported(lu);
                self.handle_intra_var_unsupported(ru);
                return;
            }
            l_ori_bv = self.icx_slice_mut().var_mut()[lu].extract();
            let l_zero_const = ast::BV::from_u64(ctx, 0, llen as u32);
            let constraint_l_ori_zero = l_ori_bv._safe_eq(&l_zero_const).unwrap();
            goal.assert(&constraint_l_ori_zero);
            solver.assert(&constraint_l_ori_zero);
            is_ctor = false;
        } else {
            // this branch means that the assignment is the constructor of the lvalue
            let r_place_ty = rplace.ty(&self.body().local_decls, self.tcx());
            let ty_with_vidx = TyWithIndex::new(r_place_ty.ty, r_place_ty.variant_index);
            match ty_with_vidx.get_priority() {
                0 => {
                    // cannot identify the ty (unsupported like fn ptr ...)
                    self.handle_intra_var_unsupported(lu);
                    self.handle_intra_var_unsupported(ru);
                    return;
                },
                1 => {
                    return;
                },
                2 => {
                    // update the layout of lvalue due to it is an instance
                    self.icx_slice_mut().ty_mut()[lu] = self.icx_slice().ty()[ru].clone();
                    self.icx_slice_mut().layout_mut()[lu] = self.icx_slice().layout()[ru].clone();
                },
                _ => unreachable!(),
            }
        }

        // update the lvalue length that is equal to rvalue
        llen = rlen;
        self.icx_slice_mut().len_mut()[lu] = llen;

        // produce the name of lvalue and rvalue in this program point
        let l_name = if is_ctor {
            new_local_name(lu, bidx, sidx).add("_ctor_asgn")
        } else {
            new_local_name(lu, bidx, sidx)
        };
        let r_name = new_local_name(ru, bidx, sidx);

        // generate new bit vectors for variables
        let l_new_bv = ast::BV::new_const(ctx, l_name, llen as u32);
        let r_new_bv = ast::BV::new_const(ctx, r_name, rlen as u32);

        let l_zero_const = ast::BV::from_u64(ctx, 0, llen as u32);
        let r_zero_const = ast::BV::from_u64(ctx, 0, rlen as u32);

        // the constraint that promise the unique ownership in transformation of y=x, l=r
        // the exactly constraint is that (r'=r && l'=0) || (l'=r && r'=0)
        // this is for (r'=r && l'=0)
        let r_owning = r_new_bv._safe_eq(&r_ori_bv).unwrap();
        let l_non_owning = l_new_bv._safe_eq(&l_zero_const).unwrap();
        let args1 = &[&r_owning, &l_non_owning];
        let summary_1 = ast::Bool::and(ctx, args1);

        // this is for (l'=r && r'=0)
        let l_owning = l_new_bv._safe_eq(&r_ori_bv).unwrap();
        let r_non_owning = r_new_bv._safe_eq(&r_zero_const).unwrap();
        let args2 = &[&l_owning, &r_non_owning];
        let summary_2 = ast::Bool::and(ctx, args2);

        // the final constraint and add the constraint to the goal of this function
        let args3 = &[&summary_1, &summary_2];
        let constraint_owning_now = ast::Bool::or(ctx, args3);

        goal.assert(&constraint_owning_now);
        solver.assert(&constraint_owning_now);

        // update the Intra var value in current basic block (exactly, the statement)
        self.icx_slice_mut().var_mut()[lu] = IntraVar::Init(l_new_bv);
        self.icx_slice_mut().var_mut()[ru] = IntraVar::Init(r_new_bv);
        self.handle_taint(lu, ru);

    }


    pub(crate) fn handle_move(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        _kind: AsgnKind,
        lplace: &Place<'tcx>,
        rplace: &Place<'tcx>,
        mut disc: Disc,
        bidx: usize,
        sidx: usize
    ) {
        let llocal = lplace.local;
        let rlocal = rplace.local;

        let lu:usize = llocal.as_usize();
        let ru:usize = rlocal.as_usize();

        // if any rvalue or lplace is unsupported, then make them all unsupported and exit
        if self.icx_slice().var()[lu].is_unsupported() || self.icx_slice.var()[ru].is_unsupported() {
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }
        if !self.icx_slice.var()[ru].is_init() {
            return;
        }

        // if the current layout of rvalue is 0, avoid any following analysis
        // e.g., a = b, b:[]
        if self.icx_slice().len()[ru] == 0 {
            // the len is 0 and ty is None which do not need update
            return;
        }

        // get the length of current variable to generate bit vector in the future
        let mut llen = self.icx_slice().len()[lu];
        let rlen = self.icx_slice().len()[ru];

        // extract the original z3 ast of the variable needed to prepare generating new
        let l_ori_bv :ast::BV;
        let r_ori_bv = self.icx_slice_mut().var_mut()[ru].extract();

        let mut is_ctor = true;
        if self.icx_slice().var()[lu].is_init() {
            // if the lvalue is not initialized for the first time
            // the constraint that promise the original value of lvalue that does not hold the ownership
            // e.g., y=move x ,that y (l) is non-owning
            // check the pointee layout (of) is same
            if self.icx_slice().ty()[lu] != self.icx_slice().ty[ru] {
                self.handle_intra_var_unsupported(lu);
                self.handle_intra_var_unsupported(ru);
                return;
            }
            l_ori_bv = self.icx_slice_mut().var_mut()[lu].extract();
            let l_zero_const = ast::BV::from_u64(ctx, 0, llen as u32);
            let constraint_l_ori_zero = l_ori_bv._safe_eq(&l_zero_const).unwrap();
            goal.assert(&constraint_l_ori_zero);
            solver.assert(&constraint_l_ori_zero);
            is_ctor = false;
        } else {
            // this branch means that the assignment is the constructor of the lvalue
            let r_place_ty = rplace.ty(&self.body().local_decls, self.tcx());
            let ty_with_vidx = TyWithIndex::new(r_place_ty.ty, r_place_ty.variant_index);
            match ty_with_vidx.get_priority() {
                0 => {
                    // cannot identify the ty (unsupported like fn ptr ...)
                    self.handle_intra_var_unsupported(lu);
                    self.handle_intra_var_unsupported(ru);
                    return;
                },
                1 => {
                    return;
                },
                2 => {
                    // update the layout of lvalue due to it is an instance
                    self.icx_slice_mut().ty_mut()[lu] = self.icx_slice().ty()[ru].clone();
                    self.icx_slice_mut().layout_mut()[lu] = self.icx_slice().layout()[ru].clone();
                },
                _ => unreachable!(),
            }
        }

        // update the lvalue length that is equal to rvalue
        llen = rlen;
        self.icx_slice_mut().len_mut()[lu] = llen;

        // produce the name of lvalue and rvalue in this program point
        let l_name = if is_ctor {
            new_local_name(lu, bidx, sidx).add("_ctor_asgn")
        } else {
            new_local_name(lu, bidx, sidx)
        };
        let r_name = new_local_name(ru, bidx, sidx);

        // generate new bit vectors for variables
        let l_new_bv = ast::BV::new_const(ctx, l_name, llen as u32);
        let r_new_bv = ast::BV::new_const(ctx, r_name, rlen as u32);

        let r_zero_const = ast::BV::from_u64(ctx, 0, rlen as u32);

        // the constraint that promise the unique ownership in transformation of y=move x, l=move r
        // the exactly constraint is that r'=0 && l'=r
        // this is for r'=0
        let r_non_owning = r_new_bv._safe_eq(&r_zero_const).unwrap();
        // this is for l'=r
        let l_owning = l_new_bv._safe_eq(&r_ori_bv).unwrap();

        goal.assert(&r_non_owning);
        goal.assert(&l_owning);
        solver.assert(&r_non_owning);
        solver.assert(&l_owning);

        // update the Intra var value in current basic block (exactly, the statement)
        self.icx_slice_mut().var_mut()[lu] = IntraVar::Init(l_new_bv);
        self.icx_slice_mut().var_mut()[ru] = IntraVar::Init(r_new_bv);
        self.handle_taint(lu, ru);

    }

    pub(crate) fn handle_copy_from_field(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        _kind: AsgnKind,
        lplace: &Place<'tcx>,
        rplace: &Place<'tcx>,
        mut disc: Disc,
        bidx: usize,
        sidx: usize
    ) {
        // y=x.f => l=r.f
        // this local of rvalue is not x.f
        let llocal = lplace.local;
        let rlocal = rplace.local;

        let lu:usize = llocal.as_usize();
        let ru:usize = rlocal.as_usize();

        // if any rvalue or lplace is unsupported, then make them all unsupported and exit
        if self.icx_slice().var()[lu].is_unsupported() || self.icx_slice.var()[ru].is_unsupported() {
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }
        if !self.icx_slice().var()[ru].is_init() {
            return;
        }

        // if the current layout of the father in rvalue is 0, avoid the following analysis
        // e.g., a = b, b:[]
        if self.icx_slice().len[ru] == 0 {
            // the len is 0 and ty is None which do not need update
            return;
        }

        // extract the ty of the rplace, the rplace has projection like _1.0
        // rpj ty is the exact ty of rplace, the first field ty of rplace
        let rpj_ty = rplace.ty(&self.body().local_decls, self.tcx());
        let rpj_fields = extract_projection(rplace);
        if rpj_fields.is_unsupported() {
            // we only support that the field depth is 1 in max
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }
        if !rpj_fields.has_field() {
            self.handle_copy(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
            return;
        }
        let index_needed = rpj_fields.index_needed();

        let default_ownership = self.extract_default_ty_layout(rpj_ty.ty, rpj_ty.variant_index);
        if !default_ownership.get_requirement() || default_ownership.is_empty() {
            return;
        }

        // get the length of current variable and the rplace projection to generate bit vector in the future
        let mut llen = self.icx_slice().len()[lu];
        let rlen = self.icx_slice().len()[ru];
        let rpj_len = default_ownership.layout().len();

        // extract the original z3 ast of the variable needed to prepare generating new
        let l_ori_bv: ast::BV;
        let r_ori_bv = self.icx_slice_mut().var_mut()[ru].extract();

        let mut is_ctor = true;
        if self.icx_slice().var()[lu].is_init() {
            // if the lvalue is not initialized for the first time
            // the constraint that promise the original value of lvalue that does not hold the ownership
            // e.g., y=move x.f ,that y (l) is non-owning
            l_ori_bv = self.icx_slice_mut().var_mut()[lu].extract();
            let l_zero_const = ast::BV::from_u64(ctx, 0, llen as u32);
            let constraint_l_ori_zero = l_ori_bv._safe_eq(&l_zero_const).unwrap();
            goal.assert(&constraint_l_ori_zero);
            solver.assert(&constraint_l_ori_zero);
            is_ctor = false;
        } else {
            // this branch means that the assignment is the constructor of the lvalue
            // Note : l = r.f => l's len must be 1 if l is a pointer
            let r_place_ty = rplace.ty(&self.body().local_decls, self.tcx());
            let ty_with_vidx = TyWithIndex::new(r_place_ty.ty, r_place_ty.variant_index);
            match ty_with_vidx.get_priority() {
                0 => {
                    // cannot identify the ty (unsupported like fn ptr ...)
                    self.handle_intra_var_unsupported(lu);
                    self.handle_intra_var_unsupported(ru);
                    return;
                },
                1 => {
                    return;
                },
                2 => {
                    // update the layout of lvalue due to it is an instance
                    self.icx_slice_mut().ty_mut()[lu] = ty_with_vidx;
                    self.icx_slice_mut().layout_mut()[lu] = default_ownership.layout().clone();
                },
                _ => unreachable!(),
            }
        }

        // update the lvalue length that is equal to rvalue
        llen = rpj_len;
        self.icx_slice_mut().len_mut()[lu] = llen;

        // produce the name of lvalue and rvalue in this program point
        let l_name = if is_ctor {
            new_local_name(lu, bidx, sidx).add("_ctor_asgn")
        } else {
            new_local_name(lu, bidx, sidx)
        };
        let r_name = new_local_name(ru, bidx, sidx);

        // generate new bit vectors for variables
        let l_new_bv = ast::BV::new_const(ctx, l_name, llen as u32);
        let r_new_bv = ast::BV::new_const(ctx, r_name, rlen as u32);

        // the constraint that promise the unique ownership in transformation of y=x.f, l=r.f
        // the exactly constraint is that ( r.f'=r.f && l'=0 ) || ( l'=extend(r.f) && r.f'=0 )
        // this is for r.f'=r.f (no change) && l'=0
        let r_f_owning = r_new_bv._safe_eq(&r_ori_bv).unwrap();
        let l_zero_const = ast::BV::from_u64(ctx, 0, llen as u32);
        let l_non_owning = l_new_bv._safe_eq(&l_zero_const).unwrap();
        let args1 = &[&r_f_owning, &l_non_owning];
        let summary_1 = ast::Bool::and(ctx, args1);

        // this is for l'=extend(r.f) && r.f'=0
        // this is for l'=extend(r.f)
        // note that we extract the ownership of the ori r.f and apply (extend) it to new lvalue
        // like l'=r.f=1 => l' [1111] and default layout [****]
        let rust_bv_for_op_and = if self.icx_slice().taint()[ru].is_tainted()
        {
            rustbv_merge(
                &ownership_layout_to_rustbv(default_ownership.layout()),
                &self.generate_ptr_layout(rpj_ty.ty, rpj_ty.variant_index)
            )
        } else {
            ownership_layout_to_rustbv(default_ownership.layout())
        };
        let int_for_op_and = rustbv_to_int(&rust_bv_for_op_and);
        let z3_bv_for_op_and = ast::BV::from_u64(ctx, int_for_op_and, llen as u32);
        let extract_from_field = r_ori_bv.extract(index_needed as u32, index_needed as u32);
        let repeat_field = if llen>1 { extract_from_field.sign_ext((llen-1) as u32) } else { extract_from_field };
        let after_op_and = z3_bv_for_op_and.bvand(&repeat_field);
        let l_extend_owning = l_new_bv._safe_eq(&after_op_and).unwrap();
        // this is for r.f'=0
        // like r.1'=0 => ori and new => [0110] and [1011] => [0010]
        // note that we calculate the index of r.f and use bit vector 'and' to update the ownership
        let mut rust_bv_for_op_and = vec![ true ; rlen ];
        rust_bv_for_op_and[index_needed] = false;
        let int_for_op_and = rustbv_to_int(&rust_bv_for_op_and);
        let z3_bv_for_op_and = ast::BV::from_u64(ctx, int_for_op_and, rlen as u32);
        let after_op_and = r_ori_bv.bvand(&z3_bv_for_op_and);
        let rpj_non_owning = r_new_bv._safe_eq(&after_op_and).unwrap();

        let args2 = &[&l_extend_owning, &rpj_non_owning];
        let summary_2 = ast::Bool::and(ctx, args2);

        // the final constraint and add the constraint to the goal of this function
        let args3 = &[&summary_1, &summary_2];
        let constraint_owning_now = ast::Bool::or(ctx, args3);

        goal.assert(&constraint_owning_now);
        solver.assert(&constraint_owning_now);

        // update the Intra var value in current basic block (exactly, the statement)
        self.icx_slice_mut().var_mut()[lu] = IntraVar::Init(l_new_bv);
        self.icx_slice_mut().var_mut()[ru] = IntraVar::Init(r_new_bv);
        self.handle_taint(lu, ru);

    }

    pub(crate) fn handle_move_from_field(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        _kind: AsgnKind,
        lplace: &Place<'tcx>,
        rplace: &Place<'tcx>,
        mut disc: Disc,
        bidx: usize,
        sidx: usize
    ) {
        // y=move x.f => l=move r.f
        // this local of rvalue is not x.f
        let llocal = lplace.local;
        let rlocal = rplace.local;

        let lu:usize = llocal.as_usize();
        let ru:usize = rlocal.as_usize();

        // if any rvalue or lplace is unsupported, then make them all unsupported and exit
        if self.icx_slice().var()[lu].is_unsupported() || self.icx_slice.var()[ru].is_unsupported() {
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }
        if !self.icx_slice().var()[ru].is_init() {
            return;
        }

        // extract the ty of the rplace, the rplace has projection like _1.0
        // rpj ty is the exact ty of rplace, the first field ty of rplace
        let rpj_ty = rplace.ty(&self.body().local_decls, self.tcx());
        let rpj_fields = extract_projection(rplace);
        if rpj_fields.is_unsupported() {
            // we only support that the field depth is 1 in max
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }
        if !rpj_fields.has_field() {
            self.handle_move(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
            return;
        }
        let index_needed = rpj_fields.index_needed();

        let default_ownership = self.extract_default_ty_layout(rpj_ty.ty, rpj_ty.variant_index);
        if !default_ownership.get_requirement() || default_ownership.is_empty() {
            return;
        }

        // get the length of current variable and the rplace projection to generate bit vector in the future
        let mut llen = self.icx_slice().len()[lu];
        let rlen = self.icx_slice().len()[ru];
        let rpj_len = default_ownership.layout().len();

        // if the current layout of the father in rvalue is 0, avoid the following analysis
        // e.g., a = b, b:[]
        if self.icx_slice().len[ru] == 0 {
            // the len is 0 and ty is None which do not need update
            return;
        }

        // extract the original z3 ast of the variable needed to prepare generating new
        let l_ori_bv: ast::BV;
        let r_ori_bv = self.icx_slice_mut().var_mut()[ru].extract();

        let mut is_ctor = true;
        if self.icx_slice().var()[lu].is_init() {
            // if the lvalue is not initialized for the first time
            // the constraint that promise the original value of lvalue that does not hold the ownership
            // e.g., y=move x.f ,that y (l) is non-owning
            // do not check the ty l = ty r due to field operation
            // if self.icx_slice().ty()[lu] != self.icx_slice().ty[ru] {
            //     self.handle_intra_var_unsupported(lu);
            //     self.handle_intra_var_unsupported(ru);
            //     return;
            // }
            l_ori_bv = self.icx_slice_mut().var_mut()[lu].extract();
            let l_zero_const = ast::BV::from_u64(ctx, 0, llen as u32);
            let constraint_l_ori_zero = l_ori_bv._safe_eq(&l_zero_const).unwrap();
            goal.assert(&constraint_l_ori_zero);
            solver.assert(&constraint_l_ori_zero);
            is_ctor = false;
        } else {
            // this branch means that the assignment is the constructor of the lvalue
            // Note : l = r.f => l's len must be 1 if l is a pointer
            let r_place_ty = rplace.ty(&self.body().local_decls, self.tcx());
            let ty_with_vidx = TyWithIndex::new(r_place_ty.ty, r_place_ty.variant_index);
            match ty_with_vidx.get_priority() {
                0 => {
                    // cannot identify the ty (unsupported like fn ptr ...)
                    self.handle_intra_var_unsupported(lu);
                    self.handle_intra_var_unsupported(ru);
                    return;
                },
                1 => {
                    return;
                },
                2 => {
                    // update the layout of lvalue due to it is an instance
                    self.icx_slice_mut().ty_mut()[lu] = ty_with_vidx;
                    self.icx_slice_mut().layout_mut()[lu] = default_ownership.layout().clone();
                },
                _ => unreachable!(),
            }
        }

        // update the lvalue length that is equal to rvalue
        llen = rpj_len;
        self.icx_slice_mut().len_mut()[lu] = llen;

        // produce the name of lvalue and rvalue in this program point
        let l_name = if is_ctor {
            new_local_name(lu, bidx, sidx).add("_ctor_asgn")
        } else {
            new_local_name(lu, bidx, sidx)
        };
        let r_name = new_local_name(ru, bidx, sidx);

        // generate new bit vectors for variables
        let l_new_bv = ast::BV::new_const(ctx, l_name, llen as u32);
        let r_new_bv = ast::BV::new_const(ctx, r_name, rlen as u32);

        // the constraint that promise the unique ownership in transformation of y=move x.f, l=move r.f
        // the exactly constraint is that l'=extend(r.f) && r.f'=0
        // this is for l'=extend(r.f)
        // note that we extract the ownership of the ori r.f and apply (extend) it to new lvalue
        // like l'=r.f=1 => l' [1111] and default layout [****]
        let rust_bv_for_op_and = if self.icx_slice().taint()[ru].is_tainted()
        {
            rustbv_merge(
                &ownership_layout_to_rustbv(default_ownership.layout()),
                &self.generate_ptr_layout(rpj_ty.ty, rpj_ty.variant_index)
            )
        } else {
            ownership_layout_to_rustbv(default_ownership.layout())
        };
        let int_for_op_and = rustbv_to_int(&rust_bv_for_op_and);
        let z3_bv_for_op_and = ast::BV::from_u64(ctx, int_for_op_and, llen as u32);
        let extract_from_field = r_ori_bv.extract(index_needed as u32, index_needed as u32);
        let repeat_field = if llen>1 { extract_from_field.sign_ext((llen-1) as u32) } else { extract_from_field };
        let after_op_and = z3_bv_for_op_and.bvand(&repeat_field);
        let l_extend_owning = l_new_bv._safe_eq(&after_op_and).unwrap();

        // this is for r.f'=0
        // like r.1'=0 => ori and new => [0110] and [1011] => [0010]
        // note that we calculate the index of r.f and use bit vector 'and' to update the ownership
        let mut rust_bv_for_op_and = vec![ true ; rlen ];
        rust_bv_for_op_and[index_needed] = false;
        let int_for_op_and = rustbv_to_int(&rust_bv_for_op_and);
        let z3_bv_for_op_and = ast::BV::from_u64(ctx, int_for_op_and, rlen as u32);
        let after_op_and = r_ori_bv.bvand(&z3_bv_for_op_and);
        let rpj_non_owning = r_new_bv._safe_eq(&after_op_and).unwrap();

        goal.assert(&l_extend_owning);
        goal.assert(&rpj_non_owning);
        solver.assert(&l_extend_owning);
        solver.assert(&rpj_non_owning);

        // update the Intra var value in current basic block (exactly, the statement)
        self.icx_slice_mut().var_mut()[lu] = IntraVar::Init(l_new_bv);
        self.icx_slice_mut().var_mut()[ru] = IntraVar::Init(r_new_bv);
        self.handle_taint(lu, ru);

    }

    pub(crate) fn handle_copy_to_field(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        _kind: AsgnKind,
        lplace: &Place<'tcx>,
        rplace: &Place<'tcx>,
        mut disc: Disc,
        bidx: usize,
        sidx: usize
    ) {
        // y.f= x => l.f= r
        // this local of lvalue is not y.f
        let llocal = lplace.local;
        let rlocal = rplace.local;

        let lu:usize = llocal.as_usize();
        let ru:usize = rlocal.as_usize();

        // if any rvalue or lplace is unsupported, then make them all unsupported and exit
        if self.icx_slice().var()[lu].is_unsupported() || self.icx_slice.var()[ru].is_unsupported() {
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }
        if !self.icx_slice().var()[ru].is_init() {
            return;
        }

        // extract the ty of the rvalue
        let l_local_ty = self.body().local_decls[llocal].ty;
        let lpj_fields = extract_projection(lplace);
        if lpj_fields.is_unsupported() {
            // we only support that the field depth is 1 in max
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }

        match (lpj_fields.has_field(), lpj_fields.has_downcast()) {
            (true, true) => {
                // .f .v => judge
                disc = lpj_fields.downcast();
                let ty_with_index = TyWithIndex::new(l_local_ty, disc);

                if ty_with_index.0.is_none() { return; }

                // variant.len = 1 && field[0]
                if lpj_fields.index_needed() == 0 && ty_with_index.0.unwrap().0 == 1 {
                    self.handle_copy(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
                    return;
                }
            },
            (true, false) => {
                // .f => normal field access
            },
            (false, true) => {
                // .v => not
                return;
            },
            (false, false) => {
                self.handle_copy(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
                return;
            },
        }

        let index_needed = lpj_fields.index_needed();

        let default_ownership = self.extract_default_ty_layout(l_local_ty, disc);
        if !default_ownership.get_requirement() || default_ownership.is_empty() {
            return;
        }

        // get the length of current variable and the lplace projection to generate bit vector in the future
        let llen = default_ownership.layout().len();
        self.icx_slice_mut().len_mut()[lu] = llen;
        let rlen = self.icx_slice().len()[ru];

        // if the current layout of the father in rvalue is 0, avoid the following analysis
        // e.g., a = b, b:[]
        if self.icx_slice().len[ru] == 0 {
            // the len is 0 and ty is None which do not need update
            return;
        }

        // extract the original z3 ast of the variable needed to prepare generating new
        let l_ori_bv: ast::BV;
        let r_ori_bv = self.icx_slice_mut().var_mut()[ru].extract();

        if self.icx_slice().var()[lu].is_init() {
            // if the lvalue is not initialized for the first time
            // the constraint that promise the original value of lvalue that does not hold the ownership
            // e.g., y.f= x ,that y.f (l) is non-owning
            l_ori_bv = self.icx_slice_mut().var_mut()[lu].extract();
            let extract_from_field = l_ori_bv.extract(index_needed as u32, index_needed as u32);
            if lu > self.body().arg_count {
                let l_f_zero_const = ast::BV::from_u64(ctx, 0, 1);
                let constraint_l_f_ori_zero = extract_from_field._safe_eq(&l_f_zero_const).unwrap();
                goal.assert(&constraint_l_f_ori_zero);
                solver.assert(&constraint_l_f_ori_zero);
            }
        } else {
            // this branch means that the assignment is the constructor of the lvalue (either l and l.f)
            // this constraint promise before the struct is [0;field]
            let l_ori_name_ctor = new_local_name(lu, bidx, sidx).add("_ctor_asgn");
            let l_ori_bv_ctor = ast::BV::new_const(ctx, l_ori_name_ctor, llen as u32);
            let l_ori_zero = ast::BV::from_u64(ctx, 0, llen as u32);
            let constraint_l_ctor_zero = l_ori_bv_ctor._safe_eq(&l_ori_zero).unwrap();
            goal.assert(&constraint_l_ctor_zero);
            solver.assert(&constraint_l_ctor_zero);
            l_ori_bv = l_ori_zero;
            self.icx_slice_mut().ty_mut()[lu] = TyWithIndex::new(l_local_ty, disc);
            self.icx_slice_mut().layout_mut()[lu] = default_ownership.layout().clone();
        }

        // we no not need to update the lvalue length that is equal to rvalue
        // llen = rlen;
        // self.icx_slice_mut().len_mut()[lu] = llen;

        // produce the name of lvalue and rvalue in this program point
        let l_name = new_local_name(lu, bidx, sidx);
        let r_name = new_local_name(ru, bidx, sidx);

        // generate new bit vectors for variables
        let l_new_bv = ast::BV::new_const(ctx, l_name, llen as u32);
        let r_new_bv = ast::BV::new_const(ctx, r_name, rlen as u32);

        let r_zero_const = ast::BV::from_u64(ctx, 0, rlen as u32);

        // the constraint that promise the unique ownership in transformation of y.f=x, l.f=r
        // the exactly constraint is that (r'=r && l.f'=0) || (r'=0 && l.f'=shrink(r))
        // this is for r'=r && l.f'=0
        // this is for r'=r
        let r_owning = r_new_bv._safe_eq(&r_ori_bv).unwrap();
        //this is for l.f'=0
        let mut rust_bv_for_op_and = vec![ true ; llen ];
        rust_bv_for_op_and[index_needed] = false;
        let int_for_op_and = rustbv_to_int(&rust_bv_for_op_and);
        let z3_bv_for_op_and =  ast::BV::from_u64(ctx, int_for_op_and, llen as u32);
        let after_op_and = l_ori_bv.bvand(&z3_bv_for_op_and);
        let lpj_non_owning = l_new_bv._safe_eq(&after_op_and).unwrap();

        let args1 = &[&r_owning, &lpj_non_owning];
        let summary_1 = ast::Bool::and(ctx, args1);

        // this is for r'=0 && l.f'=shrink(r)
        // this is for r'=0
        let r_non_owning = r_new_bv._safe_eq(&r_zero_const).unwrap();
        // this is for l.f'=shrink(r)
        // to achieve this goal would be kind of complicated
        // first we take the disjunction of whole rvalue into point as *
        // then, the we contact 3 bit vector [1;begin] [*] [1;end]
        // at last, we use and operation to simulate shrink from e.g., [0010] to [11*1]
        let disjunction_r = r_ori_bv.bvredor();
        let mut final_bv: ast::BV;

        if index_needed < llen-1 {
            let end_part = l_ori_bv.extract((llen-1) as u32, (index_needed+1) as u32);
            final_bv = end_part.concat(&disjunction_r);
        } else {
            final_bv = disjunction_r;
        }
        if index_needed > 0 {
            let begin_part = l_ori_bv.extract((index_needed-1) as u32, 0);
            final_bv = final_bv.concat(&begin_part);
        }

        let lpj_shrink_owning = l_new_bv._safe_eq(&final_bv).unwrap();

        let args2 = &[&r_non_owning, &lpj_shrink_owning];
        let summary_2 = ast::Bool::and(ctx, args2);

        // the final constraint and add the constraint to the goal of this function
        let args3 = &[&summary_1, &summary_2];
        let constraint_owning_now = ast::Bool::or(ctx, args3);

        goal.assert(&constraint_owning_now);
        solver.assert(&constraint_owning_now);

        // update the Intra var value in current basic block (exactly, the statement)
        self.icx_slice_mut().var_mut()[lu] = IntraVar::Init(l_new_bv);
        self.icx_slice_mut().var_mut()[ru] = IntraVar::Init(r_new_bv);
        self.handle_taint(lu, ru);

    }

    pub(crate) fn handle_move_to_field(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        _kind: AsgnKind,
        lplace: &Place<'tcx>,
        rplace: &Place<'tcx>,
        mut disc: Disc,
        bidx: usize,
        sidx: usize
    ) {
        // y.f=move x => l.f=move r
        // this local of lvalue is not y.f
        let llocal = lplace.local;
        let rlocal = rplace.local;

        let lu:usize = llocal.as_usize();
        let ru:usize = rlocal.as_usize();

        // if any rvalue or lplace is unsupported, then make them all unsupported and exit
        if self.icx_slice().var()[lu].is_unsupported() || self.icx_slice.var()[ru].is_unsupported() {
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }
        if !self.icx_slice().var()[ru].is_init() {
            return;
        }

        // extract the ty of the rvalue
        let l_local_ty = self.body().local_decls[llocal].ty;
        let lpj_fields = extract_projection(lplace);
        if lpj_fields.is_unsupported() {
            // we only support that the field depth is 1 in max
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }

        match (lpj_fields.has_field(), lpj_fields.has_downcast()) {
            (true, true) => {
                // .f .v => judge
                disc = lpj_fields.downcast();
                let ty_with_index = TyWithIndex::new(l_local_ty, disc);

                if ty_with_index.0.is_none() { return; }

                // variant.len = 1 && field[0]
                if lpj_fields.index_needed() == 0 && ty_with_index.0.unwrap().0 == 1 {
                    self.handle_move(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
                    return;
                }
            },
            (true, false) => {
                // .f => normal field access
            },
            (false, true) => {
                // .v => not
                return;
            },
            (false, false) => {
                self.handle_move(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
                return;
            },
        }

        let index_needed = lpj_fields.index_needed();

        let mut default_ownership = self.extract_default_ty_layout(l_local_ty, disc);
        if !default_ownership.get_requirement() || default_ownership.is_empty() {
            return;
        }

        // get the length of current variable and the lplace projection to generate bit vector in the future
        let llen = default_ownership.layout().len();
        self.icx_slice_mut().len_mut()[lu] = llen;
        let rlen = self.icx_slice().len()[ru];

        // if the current layout of the father in rvalue is 0, avoid the following analysis
        // e.g., a = b, b:[]
        if self.icx_slice().len[ru] == 0 {
            // the len is 0 and ty is None which do not need update
            return;
        }

        // extract the original z3 ast of the variable needed to prepare generating new
        let l_ori_bv: ast::BV;
        let r_ori_bv = self.icx_slice_mut().var_mut()[ru].extract();

        if self.icx_slice().var()[lu].is_init() {
            // if the lvalue is not initialized for the first time
            // the constraint that promise the original value of lvalue that does not hold the ownership
            // e.g., y.f=move x ,that y.f (l) is non-owning
            // add: y.f -> y is not argument e.g., fn(arg1) arg1.1 = 0, cause arg is init as 1
            l_ori_bv = self.icx_slice_mut().var_mut()[lu].extract();
            let extract_from_field = l_ori_bv.extract(index_needed as u32, index_needed as u32);
            if lu > self.body().arg_count {
                let l_f_zero_const = ast::BV::from_u64(ctx, 0, 1);
                let constraint_l_f_ori_zero = extract_from_field._safe_eq(&l_f_zero_const).unwrap();
                goal.assert(&constraint_l_f_ori_zero);
                solver.assert(&constraint_l_f_ori_zero);
            }
        } else {
            // this branch means that the assignment is the constructor of the lvalue (either l and l.f)
            // this constraint promise before the struct is [0;field]
            let l_ori_name_ctor = new_local_name(lu, bidx, sidx).add("_ctor_asgn");
            let l_ori_bv_ctor = ast::BV::new_const(ctx, l_ori_name_ctor, llen as u32);
            let l_ori_zero = ast::BV::from_u64(ctx, 0, llen as u32);
            let constraint_l_ctor_zero = l_ori_bv_ctor._safe_eq(&l_ori_zero).unwrap();
            goal.assert(&constraint_l_ctor_zero);
            solver.assert(&constraint_l_ctor_zero);
            l_ori_bv = l_ori_zero;
            self.icx_slice_mut().ty_mut()[lu] = TyWithIndex::new(l_local_ty, disc);
            self.icx_slice_mut().layout_mut()[lu] = default_ownership.layout_mut().clone();
        }

        // we no not need to update the lvalue length that is equal to rvalue
        // llen = rlen;
        // self.icx_slice_mut().len_mut()[lu] = llen;

        // produce the name of lvalue and rvalue in this program point
        let l_name = new_local_name(lu, bidx, sidx);
        let r_name = new_local_name(ru, bidx, sidx);

        // generate new bit vectors for variables
        let l_new_bv = ast::BV::new_const(ctx, l_name, llen as u32);
        let r_new_bv = ast::BV::new_const(ctx, r_name, rlen as u32);

        let r_zero_const = ast::BV::from_u64(ctx, 0, rlen as u32);

        // the constraint that promise the unique ownership in transformation of y.f=move x, l.f=move r
        // the exactly constraint is that r'=0 && l.f'=shrink(r)
        // this is for r'=0
        let r_non_owning = r_new_bv._safe_eq(&r_zero_const).unwrap();

        // this is for l.f'=shrink(r)
        // to achieve this goal would be kind of complicated
        // first we take the disjunction of whole rvalue into point as *
        // then, the we contact 3 bit vector [1;begin] [*] [1;end]
        // at last, we use or operation to simulate shrink from e.g., [1010] to [00*0]
        let disjunction_r = r_ori_bv.bvredor();
        let mut final_bv: ast::BV;
        if index_needed < llen-1 {
            let end_part = l_ori_bv.extract((llen-1) as u32, (index_needed+1) as u32);
            final_bv = end_part.concat(&disjunction_r);
        } else {
            final_bv = disjunction_r;
        }
        if index_needed > 0 {
            let begin_part = l_ori_bv.extract((index_needed-1) as u32, 0);
            final_bv = final_bv.concat(&begin_part);
        }
        let lpj_shrink_owning = l_new_bv._safe_eq(&final_bv).unwrap();

        goal.assert(&r_non_owning);
        goal.assert(&lpj_shrink_owning);
        solver.assert(&r_non_owning);
        solver.assert(&lpj_shrink_owning);

        // update the Intra var value in current basic block (exactly, the statement)
        self.icx_slice_mut().var_mut()[lu] = IntraVar::Init(l_new_bv);
        self.icx_slice_mut().var_mut()[ru] = IntraVar::Init(r_new_bv);
        self.handle_taint(lu, ru);

    }

    pub(crate) fn handle_copy_field_to_field(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        _kind: AsgnKind,
        lplace: &Place<'tcx>,
        rplace: &Place<'tcx>,
        disc: Disc,
        bidx: usize,
        sidx: usize
    ) {
        // y.f= x.f => l.f= r.f
        let llocal = lplace.local;
        let rlocal = rplace.local;

        let lu:usize = llocal.as_usize();
        let ru:usize = rlocal.as_usize();

        // if any rvalue or lplace is unsupported, then make them all unsupported and exit
        if self.icx_slice().var()[lu].is_unsupported() || self.icx_slice.var()[ru].is_unsupported() {
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }
        if !self.icx_slice().var()[ru].is_init() {
            return;
        }

        let l_local_ty = self.body().local_decls[llocal].ty;

        // extract the ty of the rplace, the rplace has projection like _1.0
        // rpj ty is the exact ty of rplace, the first field ty of rplace
        let rpj_ty = rplace.ty(&self.body().local_decls, self.tcx());
        let rpj_fields = extract_projection(rplace);
        if rpj_fields.is_unsupported() {
            // we only support that the field depth is 1 in max
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }

        // extract the ty of the lplace, the lplace has projection like _1.0
        // lpj ty is the exact ty of lplace, the first field ty of lplace
        let lpj_ty = lplace.ty(&self.body().local_decls, self.tcx());
        let lpj_fields = extract_projection(lplace);
        if lpj_fields.is_unsupported() {
            // we only support that the field depth is 1 in max
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }

        match (rpj_fields.has_field(), lpj_fields.has_field()) {
            (true, true) => (),
            (true, false) => {
                self.handle_copy_from_field(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
                return;
            },
            (false, true) => {
                self.handle_copy_to_field(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
                return;
            },
            (false, false) => {
                self.handle_copy(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
                return;
            },
        }

        let r_index_needed = rpj_fields.index_needed();
        let l_index_needed = lpj_fields.index_needed();

        let default_ownership = self.extract_default_ty_layout(l_local_ty, disc);
        if !default_ownership.get_requirement() || default_ownership.is_empty() {
            return;
        }

        // get the length of current variable and the rplace projection to generate bit vector in the future
        let llen = default_ownership.layout().len();
        let rlen = self.icx_slice().len()[ru];
        self.icx_slice_mut().len_mut()[lu] = llen;

        // if the current layout of the father in rvalue is 0, avoid the following analysis
        // e.g., a = b, b:[]
        if self.icx_slice().len[ru] == 0 {
            // the len is 0 and ty is None which do not need update
            return;
        }

        // extract the original z3 ast of the variable needed to prepare generating new
        let l_ori_bv: ast::BV;
        let r_ori_bv = self.icx_slice_mut().var_mut()[ru].extract();

        if self.icx_slice().var()[lu].is_init() {
            // if the lvalue is not initialized for the first time
            // the constraint that promise the original value of lvalue that does not hold the ownership
            // e.g., y.f= move x.f ,that y.f (l) is non-owning
            l_ori_bv = self.icx_slice_mut().var_mut()[lu].extract();
            let extract_from_field = l_ori_bv.extract(l_index_needed as u32, l_index_needed as u32);
            if lu > self.body().arg_count {
                let l_f_zero_const = ast::BV::from_u64(ctx, 0, 1);
                let constraint_l_f_ori_zero = extract_from_field._safe_eq(&l_f_zero_const).unwrap();
                goal.assert(&constraint_l_f_ori_zero);
                solver.assert(&constraint_l_f_ori_zero);
            }
        } else {
            // this branch means that the assignment is the constructor of the lvalue (either l and l.f)
            // this constraint promise before the struct is [0;field]
            let l_ori_name_ctor = new_local_name(lu, bidx, sidx).add("_ctor_asgn");
            let l_ori_bv_ctor = ast::BV::new_const(ctx, l_ori_name_ctor, llen as u32);
            let l_ori_zero = ast::BV::from_u64(ctx, 0, llen as u32);
            let constraint_l_ctor_zero = l_ori_bv_ctor._safe_eq(&l_ori_zero).unwrap();
            goal.assert(&constraint_l_ctor_zero);
            solver.assert(&constraint_l_ctor_zero);
            l_ori_bv = l_ori_zero;
            self.icx_slice_mut().ty_mut()[lu] = TyWithIndex::new(l_local_ty, disc);
            self.icx_slice_mut().layout_mut()[lu] = default_ownership.layout().clone();
        }

        // produce the name of lvalue and rvalue in this program point
        let l_name = new_local_name(lu, bidx, sidx);
        let r_name = new_local_name(ru, bidx, sidx);

        // generate new bit vectors for variables
        let l_new_bv = ast::BV::new_const(ctx, l_name, llen as u32);
        let r_new_bv = ast::BV::new_const(ctx, r_name, rlen as u32);

        // the constraint that promise the unique ownership in transformation of y.f= x.f, l.f= r.f
        // the exactly constraint is that (r.f'=0 && l.f'=r.f) || (l.f'=0 && r.f'=r.f)
        // this is for r.f'=0 && l.f'=r.f
        // this is for r.f'=0
        // like r.1'=0 => ori and new => [0110] and [1011] => [0010]
        // note that we calculate the index of r.f and use bit vector 'and' to update the ownership
        let mut rust_bv_for_op_and = vec![ true ; rlen ];
        rust_bv_for_op_and[r_index_needed] = false;
        let int_for_op_and = rustbv_to_int(&rust_bv_for_op_and);
        let z3_bv_for_op_and = ast::BV::from_u64(ctx, int_for_op_and, rlen as u32);
        let after_op_and = r_ori_bv.bvand(&z3_bv_for_op_and);
        let rpj_non_owning = r_new_bv._safe_eq(&after_op_and).unwrap();
        // this is for l.f'=r.f
        // to achieve this goal would be kind of complicated
        // first we extract the field from the rvalue into point as *
        // then, the we contact 3 bit vector [1;begin] [*] [1;end]
        let extract_field_r = r_ori_bv.extract(r_index_needed as u32, r_index_needed as u32);
        let mut final_bv: ast::BV;
        if l_index_needed < llen-1 {
            let end_part = l_ori_bv.extract((llen-1) as u32, (l_index_needed+1) as u32);
            final_bv = end_part.concat(&extract_field_r);
        } else {
            final_bv = extract_field_r;
        }
        if l_index_needed > 0 {
            let begin_part = l_ori_bv.extract((l_index_needed-1) as u32, 0);
            final_bv = final_bv.concat(&begin_part);
        }
        let lpj_owning = l_new_bv._safe_eq(&final_bv).unwrap();

        let args1 = &[&rpj_non_owning, &lpj_owning];
        let summary_1 = ast::Bool::and(ctx, args1);

        // this is for l.f'=0 && r.f'=r.f
        // this is for l.f'=0
        let mut rust_bv_for_op_and = vec![ true ; llen ];
        rust_bv_for_op_and[l_index_needed] = false;
        let int_for_op_and = rustbv_to_int(&rust_bv_for_op_and);
        let z3_bv_for_op_and =  ast::BV::from_u64(ctx, int_for_op_and, llen as u32);
        let after_op_and = l_ori_bv.bvand(&z3_bv_for_op_and);
        let lpj_non_owning = l_new_bv._safe_eq(&after_op_and).unwrap();
        // this is for r.f'=r.f
        let rpj_owning = r_new_bv._safe_eq(&r_ori_bv).unwrap();


        let args2 = &[&lpj_non_owning, &rpj_owning];
        let summary_2 = ast::Bool::and(ctx, args2);

        // the final constraint and add the constraint to the goal of this function
        let args3 = &[&summary_1, &summary_2];
        let constraint_owning_now = ast::Bool::or(ctx, args3);

        goal.assert(&constraint_owning_now);
        solver.assert(&constraint_owning_now);

        // update the Intra var value in current basic block (exactly, the statement)
        self.icx_slice_mut().var_mut()[lu] = IntraVar::Init(l_new_bv);
        self.icx_slice_mut().var_mut()[ru] = IntraVar::Init(r_new_bv);
        self.handle_taint(lu, ru);

    }

    pub(crate) fn handle_move_field_to_field(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        _kind: AsgnKind,
        lplace: &Place<'tcx>,
        rplace: &Place<'tcx>,
        mut disc: Disc,
        bidx: usize,
        sidx: usize
    ) {
        // y.f=move x.f => l.f=move r.f
        let llocal = lplace.local;
        let rlocal = rplace.local;

        let lu:usize = llocal.as_usize();
        let ru:usize = rlocal.as_usize();

        // if any rvalue or lplace is unsupported, then make them all unsupported and exit
        if self.icx_slice().var()[lu].is_unsupported() || self.icx_slice.var()[ru].is_unsupported() {
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }
        if !self.icx_slice().var()[ru].is_init() {
            return;
        }

        let l_local_ty = self.body().local_decls[llocal].ty;

        // extract the ty of the rplace, the rplace has projection like _1.0
        // rpj ty is the exact ty of rplace, the first field ty of rplace
        let rpj_ty = rplace.ty(&self.body().local_decls, self.tcx());
        let rpj_fields = extract_projection(rplace);
        if rpj_fields.is_unsupported() {
            // we only support that the field depth is 1 in max
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }

        // extract the ty of the lplace, the lplace has projection like _1.0
        // lpj ty is the exact ty of lplace, the first field ty of lplace
        let lpj_ty = lplace.ty(&self.body().local_decls, self.tcx());
        let lpj_fields = extract_projection(lplace);
        if lpj_fields.is_unsupported() {
            // we only support that the field depth is 1 in max
            self.handle_intra_var_unsupported(lu);
            self.handle_intra_var_unsupported(ru);
            return;
        }

        match (rpj_fields.has_field(), lpj_fields.has_field()) {
            (true, true) => (),
            (true, false) => {
                self.handle_move_from_field(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
                return;
            },
            (false, true) => {
                self.handle_move_to_field(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
            },
            (false, false) => {
                self.handle_move(ctx, goal, solver, _kind, lplace, rplace, disc, bidx, sidx);
                return;
            },
        }

        let r_index_needed = rpj_fields.index_needed();
        let l_index_needed = lpj_fields.index_needed();

        let default_ownership = self.extract_default_ty_layout(l_local_ty, disc);
        if !default_ownership.get_requirement() || default_ownership.is_empty() {
            return;
        }

        // get the length of current variable and the rplace projection to generate bit vector in the future
        let llen = default_ownership.layout().len();
        let rlen = self.icx_slice().len()[ru];
        self.icx_slice_mut().len_mut()[lu] = llen;

        // if the current layout of the father in rvalue is 0, avoid the following analysis
        // e.g., a = b, b:[]
        if self.icx_slice().len[ru] == 0 {
            // the len is 0 and ty is None which do not need update
            return;
        }

        // extract the original z3 ast of the variable needed to prepare generating new
        let l_ori_bv: ast::BV;
        let r_ori_bv = self.icx_slice_mut().var_mut()[ru].extract();

        if self.icx_slice().var()[lu].is_init() {
            // if the lvalue is not initialized for the first time
            // the constraint that promise the original value of lvalue that does not hold the ownership
            // e.g., y.f= move x.f ,that y.f (l) is non-owning
            l_ori_bv = self.icx_slice_mut().var_mut()[lu].extract();
            let extract_from_field = l_ori_bv.extract(l_index_needed as u32, l_index_needed as u32);
            if lu > self.body().arg_count {
                let l_f_zero_const = ast::BV::from_u64(ctx, 0, 1);
                let constraint_l_f_ori_zero = extract_from_field._safe_eq(&l_f_zero_const).unwrap();
                goal.assert(&constraint_l_f_ori_zero);
                solver.assert(&constraint_l_f_ori_zero);
            }
        } else {
            // this branch means that the assignment is the constructor of the lvalue (either l and l.f)
            // this constraint promise before the struct is [0;field]
            let l_ori_name_ctor = new_local_name(lu, bidx, sidx).add("_ctor_asgn");
            let l_ori_bv_ctor = ast::BV::new_const(ctx, l_ori_name_ctor, llen as u32);
            let l_ori_zero = ast::BV::from_u64(ctx, 0, llen as u32);
            let constraint_l_ctor_zero = l_ori_bv_ctor._safe_eq(&l_ori_zero).unwrap();
            goal.assert(&constraint_l_ctor_zero);
            solver.assert(&constraint_l_ctor_zero);
            l_ori_bv = l_ori_zero;
            self.icx_slice_mut().ty_mut()[lu] = TyWithIndex::new(l_local_ty, disc);
            self.icx_slice_mut().layout_mut()[lu] = default_ownership.layout().clone();
        }

        // produce the name of lvalue and rvalue in this program point
        let l_name = new_local_name(lu, bidx, sidx);
        let r_name = new_local_name(ru, bidx, sidx);

        // generate new bit vectors for variables
        let l_new_bv = ast::BV::new_const(ctx, l_name, llen as u32);
        let r_new_bv = ast::BV::new_const(ctx, r_name, rlen as u32);

        // the constraint that promise the unique ownership in transformation of y.f=move x.f, l.f=move r.f
        // the exactly constraint is that r.f'=0 && l.f'=r.f
        // this is for r.f'=0
        // like r.1'=0 => ori and new => [0110] and [1011] => [0010]
        // note that we calculate the index of r.f and use bit vector 'and' to update the ownership
        let mut rust_bv_for_op_and = vec![ true ; rlen ];
        rust_bv_for_op_and[r_index_needed] = false;
        let int_for_op_and = rustbv_to_int(&rust_bv_for_op_and);
        let z3_bv_for_op_and = ast::BV::from_u64(ctx, int_for_op_and, rlen as u32);
        let after_op_and = r_ori_bv.bvand(&z3_bv_for_op_and);
        let rpj_non_owning = r_new_bv._safe_eq(&after_op_and).unwrap();

        // this is for l.f'=r.f
        // to achieve this goal would be kind of complicated
        // first we extract the field from the rvalue into point as *
        // then, the we contact 3 bit vector [1;begin] [*] [1;end]
        let extract_field_r = r_ori_bv.extract(r_index_needed as u32, r_index_needed as u32);
        let mut final_bv: ast::BV;

        if l_index_needed < llen-1 {
            let end_part = l_ori_bv.extract((llen-1) as u32, (l_index_needed+1) as u32);
            final_bv = end_part.concat(&extract_field_r);
        } else {
            final_bv = extract_field_r;
        }
        if l_index_needed > 0 {
            let begin_part = l_ori_bv.extract((l_index_needed-1) as u32, 0);
            final_bv = final_bv.concat(&begin_part);
        }
        let lpj_owning = l_new_bv._safe_eq(&final_bv).unwrap();

        goal.assert(&rpj_non_owning);
        goal.assert(&lpj_owning);
        solver.assert(&rpj_non_owning);
        solver.assert(&lpj_owning);

        // update the Intra var value in current basic block (exactly, the statement)
        self.icx_slice_mut().var_mut()[lu] = IntraVar::Init(l_new_bv);
        self.icx_slice_mut().var_mut()[ru] = IntraVar::Init(r_new_bv);
        self.handle_taint(lu, ru);

    }

    pub(crate) fn handle_stmt_live(
        &mut self,
        local: &Local,
        bidx: usize
    ) {
        // self
        //     .icx_slice_mut()
        //     .storage_mut()
        //     [local.index()]
        //     = Storage::Declared;
    }

    pub(crate) fn handle_stmt_dead(
        &mut self,
        local: &Local,
        bidx: usize
    ) {
        // self
        //     .icx_slice_mut()
        //     .storage_mut()
        //     [local.index()]
        //     = Storage::Dead;
    }

    pub(crate) fn check_fn_source(
        &mut self,
        args: &Vec<Operand<'tcx>>,
        dest: &Place<'tcx>,
    ) -> bool {

        if args.len() != 1 { return false; }

        let l_place_ty = dest.ty(&self.body().local_decls, self.tcx());
        if !is_place_containing_ptr(&l_place_ty.ty) {
            return false;
        }

        match args[0] {
            Operand::Move(aplace) => {
                let a_place_ty = aplace.ty(&self.body().local_decls, self.tcx());
                let default_layout = self.extract_default_ty_layout(a_place_ty.ty, a_place_ty.variant_index);
                if default_layout.is_owned() {
                    self.taint_flag = true;
                    true
                } else {
                    false
                }
            },
            _ => false,
        }

    }

    pub(crate) fn check_fn_recovery(
        &mut self,
        args: &Vec<Operand<'tcx>>,
        dest: &Place<'tcx>,
    ) -> (bool, Vec<usize>) {

        let mut ans:(bool, Vec<usize>) = (false, Vec::new());

        if args.len() == 0 { return ans; }

        let l_place_ty = dest.ty(&self.body().local_decls, self.tcx());
        let default_layout = self.extract_default_ty_layout(l_place_ty.ty, l_place_ty.variant_index);
        if !default_layout.get_requirement() || default_layout.is_empty() {
            return ans;
        }
        let ty_with_idx = TyWithIndex::new(l_place_ty.ty, l_place_ty.variant_index);

        for arg in args {
            match arg {
                Operand::Move(aplace) => {
                    let au:usize = aplace.local.as_usize();
                    let taint = &self.icx_slice().taint()[au];
                    if taint.is_tainted() && taint.contains(&ty_with_idx) {
                        ans.0 = true;
                        ans.1.push(au);
                    }
                },
                Operand::Copy(aplace) => {
                    let au:usize = aplace.local.as_usize();
                    let taint = &self.icx_slice().taint()[au];
                    if taint.is_tainted() && taint.contains(&ty_with_idx) {
                        ans.0 = true;
                        ans.1.push(au);
                    }
                },
                _ => (),
            }
        }
        ans
    }

    pub(crate) fn handle_call(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        term: Terminator<'tcx>,
        func: &Operand<'tcx>,
        args: &Vec<Operand<'tcx>>,
        dest: &Place<'tcx>,
        bidx: usize,
    ) {

        match func {
            Operand::Constant(constant) => {
                match constant.ty().kind() {
                    ty::FnDef(id, ..) => {
                        //println!("{:?}", id);
                        //println!("{:?}", mir_body(self.tcx(), *id));
                        match id.index.as_usize() {
                            2171 => {
                                // this for calling std::mem::drop(TY)
                                match args[0] {
                                    Operand::Move(aplace) => {
                                        let a_place_ty = dest.ty(&self.body().local_decls, self.tcx());
                                        let a_ty = a_place_ty.ty;
                                        if a_ty.is_adt() {
                                            self.handle_drop(ctx, goal, solver, &aplace, bidx, false);
                                            return;
                                        }
                                    }, _ => (),
                                }

                            }, _ => (),
                        }
                    }, _ => (),
                }
            }, _ => (),
        }

        // for return value
        let llocal = dest.local;
        let lu:usize = llocal.as_usize();

        // the source flag is for fn(self) -> */&
        // we will tag the lvalue as tainted and change the default ctor to modified one
        let source_flag = self.check_fn_source(args, dest);
        // the recovery flag is for fn(*) -> Self
        // the return value should have the same layout as tainted one
        // we will take the ownership of the args if the arg is a pointer
        let recovery_flag = self.check_fn_recovery(args, dest);
        if source_flag { self.add_taint(term); }

        for arg in args {
            match arg {
                Operand::Move(aplace) => {

                    let alocal = aplace.local;
                    let au:usize = alocal.as_usize();

                    // if the current layout of the father in rvalue is 0, avoid the following analysis
                    // e.g., a = b, b:[]
                    if self.icx_slice().len()[au] == 0 {
                        // the len is 0 and ty is None which do not need update
                        continue;
                    }

                    if !self.icx_slice().var()[au].is_init() {
                        continue;
                    }

                    let a_place_ty = aplace.ty(&self.body().local_decls, self.tcx());
                    let a_ty = a_place_ty.ty;
                    let is_a_ptr = a_ty.is_any_ptr();

                    let a_ori_bv = self.icx_slice_mut().var_mut()[au].extract();
                    let alen = self.icx_slice().len()[au];

                    if source_flag {
                        self.icx_slice_mut().taint_mut()[lu].insert(
                            TyWithIndex::new(
                                a_place_ty.ty,
                                a_place_ty.variant_index
                            )
                        );
                    }

                    match aplace.projection.len() {
                        0 => {
                            // this indicates that the operand is move without projection
                            if is_a_ptr {
                                if recovery_flag.0 && recovery_flag.1.contains(&au) {
                                    self.handle_drop(ctx, goal, solver, aplace, bidx, true);
                                    continue;
                                }

                                // if the aplace is a pointer (move ptr => still hold)
                                // the exact constraint is a=0, a'=a
                                // this is for a=0
                                let a_zero_const = ast::BV::from_u64(ctx, 0, alen as u32);
                                let a_ori_non_owing = a_ori_bv._safe_eq(&a_zero_const).unwrap();

                                // this is for a'=a
                                let a_name = new_local_name(au, bidx, 0).add("_param_pass");
                                let a_new_bv = ast::BV::new_const(ctx, a_name, alen as u32);
                                let update_a = a_new_bv._safe_eq(&a_ori_bv).unwrap();

                                goal.assert(&a_ori_non_owing);
                                goal.assert(&update_a);
                                solver.assert(&a_ori_non_owing);
                                solver.assert(&update_a);

                                self.icx_slice_mut().var_mut()[au] = IntraVar::Init(a_new_bv);
                            } else {
                                // if the aplace is a instance (move i => drop)
                                self.handle_drop(ctx, goal, solver, aplace, bidx, false);
                            }
                        },
                        1 => {
                            // this indicates that the operand is move without projection
                            if is_a_ptr {
                                if recovery_flag.0 && recovery_flag.1.contains(&au) {
                                    self.handle_drop(ctx, goal, solver, aplace, bidx, true);
                                    continue;
                                }
                                // if the aplace in field is a pointer (move a.f (ptr) => still hold)
                                // the exact constraint is a'=a
                                // this is for a'=a
                                let a_name = new_local_name(au, bidx, 0).add("_param_pass");
                                let a_new_bv = ast::BV::new_const(ctx, a_name, alen as u32);
                                let update_a = a_new_bv._safe_eq(&a_ori_bv).unwrap();

                                goal.assert(&update_a);
                                solver.assert(&update_a);
                            } else {
                                // if the aplace is a instance (move i.f => i.f=0)
                                self.handle_drop(ctx, goal, solver, aplace, bidx, false);
                            }
                        },
                        _ => {
                            self.handle_intra_var_unsupported(au);
                            continue;
                        }
                    }

                },
                Operand::Copy(aplace) => {

                    let alocal = aplace.local;
                    let au:usize = alocal.as_usize();

                    // if the current layout of the father in rvalue is 0, avoid the following analysis
                    // e.g., a = b, b:[]
                    if self.icx_slice().len()[au] == 0 {
                        // the len is 0 and ty is None which do not need update
                        continue;
                    }

                    if !self.icx_slice().var()[au].is_init() {
                        continue;
                    }

                    let a_ty = aplace.ty(&self.body().local_decls, self.tcx()).ty;
                    let is_a_ptr = a_ty.is_any_ptr();

                    let a_ori_bv = self.icx_slice_mut().var_mut()[au].extract();
                    let alen = self.icx_slice().len()[au];

                    match aplace.projection.len() {
                        0 => {
                            // this indicates that the operand is move without projection
                            if is_a_ptr {

                                if recovery_flag.0 && recovery_flag.1.contains(&au) {
                                    self.handle_drop(ctx, goal, solver, aplace, bidx, true);
                                    continue;
                                }

                                // if the aplace is a pointer (ptr => still hold)
                                // the exact constraint is a=0, a'=a
                                // this is for a=0
                                let a_zero_const = ast::BV::from_u64(ctx, 0, alen as u32);
                                let a_ori_non_owing = a_ori_bv._safe_eq(&a_zero_const).unwrap();

                                // this is for a'=a
                                let a_name = new_local_name(au, bidx, 0).add("_param_pass");
                                let a_new_bv = ast::BV::new_const(ctx, a_name, alen as u32);
                                let update_a = a_new_bv._safe_eq(&a_ori_bv).unwrap();

                                goal.assert(&a_ori_non_owing);
                                goal.assert(&update_a);
                                solver.assert(&a_ori_non_owing);
                                solver.assert(&update_a);

                                self.icx_slice_mut().var_mut()[au] = IntraVar::Init(a_new_bv);

                            } else {
                                // if the aplace is a instance (i => Copy)
                                // for Instance Copy => No need to change

                                if is_a_ptr {
                                    if recovery_flag.0 && recovery_flag.1.contains(&au) {
                                        self.handle_drop(ctx, goal, solver, aplace, bidx, true);
                                        continue;
                                    }
                                }

                                let a_name = new_local_name(au, bidx, 0).add("_param_pass");
                                let a_new_bv = ast::BV::new_const(ctx, a_name, alen as u32);
                                let update_a = a_new_bv._safe_eq(&a_ori_bv).unwrap();

                                goal.assert(&update_a);
                                solver.assert(&update_a);
                            }
                        },
                        1 => {
                            // this indicates that the operand is move without projection
                            let a_name = new_local_name(au, bidx, 0).add("_param_pass");
                            let a_new_bv = ast::BV::new_const(ctx, a_name, alen as u32);
                            let update_a = a_new_bv._safe_eq(&a_ori_bv).unwrap();

                            goal.assert(&update_a);
                            solver.assert(&update_a);
                        },
                        _ => {
                            self.handle_intra_var_unsupported(au);
                            continue;
                        }
                    }
                },
                Operand::Constant( .. ) => continue,
            }
        }

        // establish constraints for return value
        if self.icx_slice().var()[lu].is_unsupported() {
            self.handle_intra_var_unsupported(lu);
            return;
        }

        let l_ori_bv: ast::BV;

        let l_place_ty = dest.ty(&self.body().local_decls, self.tcx());
        let variant_idx = match l_place_ty.variant_index {
            Some(vidx) => Some(vidx.index()),
            None => None,
        };
        let l_ty = l_place_ty.ty;
        let l_local_ty = self.body().local_decls[llocal].ty;

        let mut is_ctor = true;
        match dest.projection.len() {

            0 => {
                // alike move instance

                let return_value_layout = self.extract_default_ty_layout(l_place_ty.ty, l_place_ty.variant_index);
                if return_value_layout.is_empty() || !return_value_layout.get_requirement() {
                    return;
                }

                let int_for_gen = if source_flag {
                    let modified_layout_bv = self.generate_ptr_layout(l_place_ty.ty, l_place_ty.variant_index);
                    let merge_layout_bv = rustbv_merge(
                        &ownership_layout_to_rustbv(return_value_layout.layout()),
                        &modified_layout_bv
                    );
                    rustbv_to_int(&merge_layout_bv)
                } else {
                    rustbv_to_int(&ownership_layout_to_rustbv(return_value_layout.layout()))
                };

                let mut llen = self.icx_slice().len()[lu];

                if self.icx_slice().var()[lu].is_init() {
                    l_ori_bv = self.icx_slice_mut().var_mut()[lu].extract();
                    let l_zero_const = ast::BV::from_u64(ctx, 0, llen as u32);
                    let constraint_l_ori_zero = l_ori_bv._safe_eq(&l_zero_const).unwrap();
                    goal.assert(&constraint_l_ori_zero);
                    solver.assert(&constraint_l_ori_zero);
                    is_ctor = false;
                } else {
                    // this branch means that the assignment is the constructor of the lvalue
                    let ty_with_vidx = TyWithIndex::new(l_place_ty.ty, l_place_ty.variant_index);
                    match ty_with_vidx.get_priority() {
                        0 => {
                            // cannot identify the ty (unsupported like fn ptr ...)
                            self.handle_intra_var_unsupported(lu);
                            return;
                        },
                        1 => {
                            return;
                        },
                        2 => {
                            // update the layout of lvalue due to it is an instance
                            self.icx_slice_mut().ty_mut()[lu] = ty_with_vidx;
                            self.icx_slice_mut().layout_mut()[lu] = return_value_layout.layout().clone();
                        },
                        _ => unreachable!(),
                    }
                }

                llen = return_value_layout.layout().len();

                let l_name = if is_ctor {
                    new_local_name(lu, bidx, 0).add("_ctor_fn")
                } else {
                    new_local_name(lu, bidx, 0).add("_cover_fn")
                };

                let l_layout_bv = ast::BV::from_u64(ctx, int_for_gen, llen as u32);
                let l_new_bv = ast::BV::new_const(ctx, l_name, llen as u32);

                let constraint_new_owning = l_new_bv._safe_eq(&l_layout_bv).unwrap();

                goal.assert(&constraint_new_owning);
                solver.assert(&constraint_new_owning);

                self.icx_slice_mut().len_mut()[lu] =llen;
                self.icx_slice_mut().var_mut()[lu] = IntraVar::Init(l_new_bv);
            },
            1 => {
                // alike move to field

                let return_value_layout = self.extract_default_ty_layout(l_local_ty, None);
                if return_value_layout.is_empty() || !return_value_layout.get_requirement() {
                    return;
                }

                let int_for_gen = rustbv_to_int(&ownership_layout_to_rustbv(return_value_layout.layout()));

                let llen = self.icx_slice().len()[lu];

                let lpj_fields = extract_projection(dest);
                let index_needed = lpj_fields.index_needed();

                if self.icx_slice().var()[lu].is_init() {
                    l_ori_bv = self.icx_slice_mut().var_mut()[lu].extract();
                    let extract_from_field = l_ori_bv.extract(index_needed as u32, index_needed as u32);
                    let l_f_zero_const = ast::BV::from_u64(ctx, 0, 1);
                    let constraint_l_f_ori_zero = extract_from_field._safe_eq(&l_f_zero_const).unwrap();

                    goal.assert(&constraint_l_f_ori_zero);
                    solver.assert(&constraint_l_f_ori_zero);
                } else {
                    let l_ori_name_ctor = new_local_name(lu, bidx, 0).add("_ctor_fn");
                    let l_ori_bv_ctor = ast::BV::new_const(ctx, l_ori_name_ctor, llen as u32);
                    let l_ori_zero = ast::BV::from_u64(ctx, 0, llen as u32);
                    let constraint_l_ctor_zero = l_ori_bv_ctor._safe_eq(&l_ori_zero).unwrap();

                    goal.assert(&constraint_l_ctor_zero);
                    solver.assert(&constraint_l_ctor_zero);

                    l_ori_bv = l_ori_zero;
                    self.icx_slice_mut().ty_mut()[lu] = TyWithIndex::new(l_local_ty, None);
                    self.icx_slice_mut().layout_mut()[lu] = return_value_layout.layout().clone();
                }

                let l_name = new_local_name(lu, bidx, 0);
                let l_new_bv = ast::BV::new_const(ctx, l_name, llen as u32);

                let update_field = if source_flag {
                    ast::BV::from_u64(ctx, 1, 1)
                } else {
                    if return_value_layout.layout()[index_needed] == RawTypeOwner::Owned {
                        ast::BV::from_u64(ctx, 1, 1)
                    } else {
                        ast::BV::from_u64(ctx, 0, 1)
                    }
                };

                let mut final_bv: ast::BV;
                if index_needed < llen-1 {
                    let end_part = l_ori_bv.extract((llen-1) as u32, (index_needed+1) as u32);
                    final_bv = end_part.concat(&update_field);
                } else {
                    final_bv = update_field;
                }
                if index_needed > 0 {
                    let begin_part = l_ori_bv.extract((index_needed-1) as u32, 0);
                    final_bv = final_bv.concat(&begin_part);
                }
                let update_filed_using_func = l_new_bv._safe_eq(&final_bv).unwrap();

                goal.assert(&update_filed_using_func);
                solver.assert(&update_filed_using_func);

                self.icx_slice_mut().len_mut()[lu] = return_value_layout.layout().len();
                self.icx_slice_mut().var_mut()[lu] = IntraVar::Init(l_new_bv);
            },
            _ => {
                self.handle_intra_var_unsupported(lu);
                return;
            }
        }
    }

    pub(crate) fn handle_return(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        sw: &Stopwatch,
        bidx: usize,
    ) {

        let place_0 = Place::from(Local::from_usize(0));
        self.handle_drop(
            ctx,
            goal,
            solver,
            &place_0,
            bidx,
            false,
        );

        // when whole function return => we need to check every variable is freed
        for (iidx, var) in self.icx_slice().var.iter().enumerate() {
            let len = self.icx_slice().len()[iidx];
            if len == 0 { continue; }
            if iidx <= self.body().arg_count { continue; }

            if var.is_init() {

                let var_ori_bv = var.extract();

                let return_name = new_local_name(iidx, bidx, 0).add("_return");
                let var_return_bv = ast::BV::new_const(ctx, return_name, len as u32);

                let zero_const = ast::BV::from_u64(ctx, 0, len as u32);

                let var_update = var_return_bv._safe_eq(&var_ori_bv).unwrap();
                let var_freed = var_return_bv._safe_eq(&zero_const).unwrap();

                let args = &[&var_update, &var_freed];
                let constraint_return = ast::Bool::and(ctx, args);

                goal.assert(&constraint_return);
                solver.assert(&constraint_return);
            }
        }

        let sec_build = sw.elapsed_ms();

        let result = solver.check();
        let model = solver.get_model();

        let sec_solve = sw.elapsed_ms() - sec_build;

        // self.add_time_build(sec_build);
        // self.add_time_solve(sec_solve);

        if is_z3_goal_verbose() {
            let g = format!("{}", goal);
            println!("{}\n", g.color(Color::LightGray).bold());
            if model.is_some() {
                println!("{}", format!("{}", model.unwrap()).color(Color::LightCyan).bold());
            }
        }

        // println!("{}", self.body().local_decls.display());
        // println!("{}", self.body().basic_blocks.display());
        // let g = format!("{}", goal);
        // println!("{}\n", g.color(Color::LightGray).bold());


        if result == z3::SatResult::Unsat && self.taint_flag {
            rap_warn!("{}", format!("RCanary: Leak Function: {:?} {:?} {:?}", result,self.did(), self.body().span));
            for source in self.taint_source.iter() {
                rap_warn!("{}", format!("RCanary: LeakItem Candidates: {:?}, {:?}", source.kind, source.source_info.span));
            }
        }

    }

    pub(crate) fn handle_drop(
        &mut self,
        ctx: &'ctx z3::Context,
        goal: &'ctx z3::Goal<'ctx>,
        solver: &'ctx z3::Solver<'ctx>,
        dest: &Place<'tcx>,
        bidx: usize,
        recovery: bool,
    ) {
        let local = dest.local;
        let u:usize = local.as_usize();

        if self.icx_slice().len()[u] == 0 {
            return;
        }

        if self.icx_slice().var()[u].is_declared() || self.icx_slice().var()[u].is_unsupported() {
            return;
        }

        let len = self.icx_slice().len()[u];
        let rust_bv = reverse_ownership_layout_to_rustbv(&self.icx_slice().layout()[u]);
        let ori_bv = self.icx_slice().var()[u].extract();

        let f = extract_projection(dest);
        if f.is_unsupported() {
            self.handle_intra_var_unsupported(u);
            return;
        }

        match f.has_field() {
            false => {
                // drop the entire owning item
                // reverse the ownership layout and using and operator
                if recovery {
                    // recovery for pointer, clear all
                    let name = new_local_name(u, bidx, 0).add("_drop_recovery");
                    let new_bv = ast::BV::new_const(ctx, name, len as u32);
                    let zero_bv = ast::BV::from_u64(ctx, 0, len as u32);

                    let and_bv = ori_bv.bvand(&zero_bv);

                    let constraint_recovery = new_bv._eq(&and_bv);

                    goal.assert(&constraint_recovery);
                    solver.assert(&constraint_recovery);

                    self.icx_slice_mut().var_mut()[u] = IntraVar::Init(new_bv);
                } else {
                    // is not recovery for pointer, just normal drop
                    let name = new_local_name(u, bidx, 0).add("_drop_all");
                    let new_bv = ast::BV::new_const(ctx, name, len as u32);
                    let int_for_rust_bv = rustbv_to_int(&rust_bv);
                    let int_bv_const = ast::BV::from_u64(ctx, int_for_rust_bv, len as u32);

                    let and_bv = ori_bv.bvand(&int_bv_const);

                    let constraint_reverse = new_bv._eq(&and_bv);

                    goal.assert(&constraint_reverse);
                    solver.assert(&constraint_reverse);

                    self.icx_slice_mut().var_mut()[u] = IntraVar::Init(new_bv);
                }
            },
            true => {
                // drop the field
                let index_needed = f.index_needed();

                if index_needed >= rust_bv.len() { return; }

                let name = if recovery {
                    new_local_name(u, bidx, 0).add("_drop_f_recovery")
                } else {
                    new_local_name(u, bidx, 0).add("_drop_f")
                };
                let new_bv = ast::BV::new_const(ctx, name, len as u32);

                if (rust_bv[index_needed] && !recovery) || (!rust_bv[index_needed] && recovery) {
                    // not actually drop, just update the idx
                    // the default ownership is false (non-owning) somehow, we just reverse it before
                    let constraint_update = new_bv._eq(&ori_bv);

                    goal.assert(&constraint_update);
                    solver.assert(&constraint_update);

                    self.icx_slice_mut().var_mut()[u] = IntraVar::Init(new_bv);
                } else {
                    let f_free = ast::BV::from_u64(ctx, 0, 1);
                    let mut final_bv: ast::BV;
                    if index_needed < len-1 {
                        let end_part = ori_bv.extract((len-1) as u32, (index_needed+1) as u32);
                        final_bv = end_part.concat(&f_free);
                    } else {
                        final_bv = f_free;
                    }
                    if index_needed > 0 {
                        let begin_part = ori_bv.extract((index_needed-1) as u32, 0);
                        final_bv = final_bv.concat(&begin_part);
                    }

                    let constraint_free_f = new_bv._safe_eq(&final_bv).unwrap();

                    goal.assert(&constraint_free_f);
                    solver.assert(&constraint_free_f);

                    self.icx_slice_mut().var_mut()[u] = IntraVar::Init(new_bv);
                }

            },
        }

    }


    pub(crate) fn handle_intra_var_unsupported(
        &mut self,
        idx: usize
    ) {
        match self.icx_slice_mut().var_mut()[idx] {
            IntraVar::Unsupported => return,
            IntraVar::Declared
            | IntraVar::Init(_) => {
                // turns into the unsupported
                self.icx_slice_mut().var_mut()[idx] = IntraVar::Unsupported;
                self.icx_slice_mut().len_mut()[idx] = 0;
                return ;
            } ,
        }
    }

    pub(crate) fn handle_taint(&mut self, l: usize, r: usize) {
        if self.icx_slice().taint()[r].is_untainted() {
            return;
        }

        if self.icx_slice().taint()[l].is_untainted() {
            self.icx_slice_mut().taint_mut()[l] = self.icx_slice().taint()[r].clone();
        } else {
            for elem in self.icx_slice().taint()[r].set().clone() {
                self.icx_slice_mut().taint_mut()[l].insert(elem);
            }
        }

    }

    pub(crate) fn extract_default_ty_layout(
        &mut self,
        ty: Ty<'tcx>,
        variant: Option<VariantIdx>
    ) -> OwnershipLayoutResult {
        match ty.kind() {
            TyKind::Array( .. ) => {
                let mut res = OwnershipLayoutResult::new();
                let mut default_ownership = DefaultOwnership::new(self.tcx(), self.owner());

                ty.visit_with(&mut default_ownership);
                res.update_from_default_ownership_visitor(&mut default_ownership);

                res
            },
            TyKind::Tuple( tuple_ty_list ) => {
                let mut res = OwnershipLayoutResult::new();

                for tuple_ty in tuple_ty_list.iter() {
                    let mut default_ownership = DefaultOwnership::new(self.tcx(), self.owner());

                    tuple_ty.visit_with(&mut default_ownership);
                    res.update_from_default_ownership_visitor(&mut default_ownership);
                }

                res
            },
            TyKind::Adt( adtdef, substs ) => {
                // check the ty is or is not an enum and the variant of this enum is or is not given
                if adtdef.is_enum() && variant.is_none() {
                    return OwnershipLayoutResult::new();
                }

                let mut res = OwnershipLayoutResult::new();

                // check the ty if it is a struct or union
                if adtdef.is_struct() || adtdef.is_union() {
                    for field in adtdef.all_fields() {
                        let field_ty = field.ty(self.tcx(), substs);

                        let mut default_ownership = DefaultOwnership::new(self.tcx(), self.owner());

                        field_ty.visit_with(&mut default_ownership);
                        res.update_from_default_ownership_visitor(&mut default_ownership);
                    }
                }
                // check the ty which is an enum with a exact variant idx
                else if adtdef.is_enum() {
                    let vidx = variant.unwrap();

                    for field in &adtdef.variants()[vidx].fields {
                        let field_ty = field.ty(self.tcx(), substs);

                        let mut default_ownership = DefaultOwnership::new(self.tcx(), self.owner());

                        field_ty.visit_with(&mut default_ownership);
                        res.update_from_default_ownership_visitor(&mut default_ownership);
                    }
                }
                res
            },
            TyKind::Param( .. ) => {
                let mut res = OwnershipLayoutResult::new();
                res.set_requirement(true);
                res.set_param(true);
                res.set_owned(true);
                res.layout_mut().push(RawTypeOwner::Owned);
                res
            },
            TyKind::RawPtr( .. ) => {
                let mut res = OwnershipLayoutResult::new();
                res.set_requirement(true);
                res.layout_mut().push(RawTypeOwner::Unowned);
                res
            },
            TyKind::Ref( .. ) => {
                let mut res = OwnershipLayoutResult::new();
                res.set_requirement(true);
                res.layout_mut().push(RawTypeOwner::Unowned);
                res
            },
            _ => {
                OwnershipLayoutResult::new()
            },
        }
    }

    pub(crate) fn generate_ptr_layout(
        &mut self,
        ty: Ty<'tcx>,
        variant: Option<VariantIdx>
    ) -> RustBV {

        let mut res = Vec::new();
        match ty.kind() {
            TyKind::Array( .. ) => {
                res.push(false);
                res
            },
            TyKind::Tuple( tuple_ty_list ) => {
                for tuple_ty in tuple_ty_list.iter() {
                    if tuple_ty.is_any_ptr() {
                        res.push(true);
                    } else {
                        res.push(false);
                    }
                }

                res
            },
            TyKind::Adt( adtdef, substs ) => {
                // check the ty is or is not an enum and the variant of this enum is or is not given
                if adtdef.is_enum() && variant.is_none() {
                    return res;
                }

                // check the ty if it is a struct or union
                if adtdef.is_struct() || adtdef.is_union() {
                    for field in adtdef.all_fields() {
                        res.push(false);
                    }
                }
                // check the ty which is an enum with a exact variant idx
                else if adtdef.is_enum() {
                    let vidx = variant.unwrap();

                    for field in &adtdef.variants()[vidx].fields {
                        res.push(false);
                    }
                }
                res
            },
            TyKind::Param( .. ) => {
                res.push(false);
                res
            },
            TyKind::RawPtr( .. ) => {
                res.push(true);
                res
            },
            TyKind::Ref( .. ) => {
                res.push(true);
                res
            },
            _ => {
                res
            },
        }

    }
}

fn new_local_name(local: usize, bidx: usize, sidx: usize) -> String {
    let s = bidx.to_string()
        .add("_")
        .add(&sidx.to_string())
        .add("_")
        .add(&local.to_string());
    s
}

fn is_place_containing_ptr(ty: &Ty) -> bool {
    match ty.kind() {
        TyKind::Tuple( tuple_ty_list ) => {
            for tuple_ty in tuple_ty_list.iter() {
                if tuple_ty.is_any_ptr() {
                    return true;
                }
            }
            false
        },
        TyKind::RawPtr( .. ) => true,
        TyKind::Ref( .. ) => true,
        _ => false,
    }
}

#[derive(Debug)]
struct ProjectionSupport<'tcx>  {
    pf_vec: Vec<(usize, Ty<'tcx>)>,
    deref: bool,
    downcast: Disc,
    unsupport: bool,
}

impl<'tcx> Default for ProjectionSupport<'tcx> {
    fn default() -> Self {
        Self {
            pf_vec: Vec::default(),
            deref: false,
            downcast: None,
            unsupport: false,
        }
    }
}

impl<'tcx> ProjectionSupport<'tcx> {
    pub fn pf_push(&mut self, index: usize, ty: Ty<'tcx>) {
        self.pf_vec.push((index, ty));
    }

    pub fn is_unsupported(&self) -> bool {
        self.unsupport == true
    }

    pub fn has_field(&self) -> bool {
        self.pf_vec.len() > 0
    }

    pub fn has_downcast(&self) -> bool {
        self.downcast.is_some()
    }

    pub fn downcast(&self) -> Disc {
        self.downcast
    }

    pub fn index_needed(&self) -> usize {
        self.pf_vec[0].0
    }

}

fn has_projection(place: &Place) -> bool {
    return if place.projection.len() > 0 { true } else { false }
}

fn extract_projection<'tcx>(place: &Place<'tcx>) -> ProjectionSupport<'tcx> {
    // extract the field index of the place
    // if the ProjectionElem we find the variant is not Field, stop it and exit
    // for field sensitivity analysis only
    let mut ans:ProjectionSupport<'tcx> = ProjectionSupport::default();
    for (idx, each_pj) in place.projection.iter().enumerate() {
        match each_pj {
            ProjectionElem::Field(field, ty) => {
                ans.pf_push(field.index(), ty);
                if ans.pf_vec.len() > 1 { ans.unsupport = true; break; }
                if ans.deref { ans.unsupport = true; break; }
            },
            ProjectionElem::Deref => {
                ans.deref = true;
                if idx > 0 { ans.unsupport = true; break; }
            },
            ProjectionElem::Downcast(.., ref vidx) => {
                ans.downcast = Some(*vidx);
                if idx > 0 { ans.unsupport = true; break; }
            },
            ProjectionElem::ConstantIndex { .. }
            | ProjectionElem::Subslice { .. }
            | ProjectionElem::Index ( .. )
            | ProjectionElem::OpaqueCast ( .. )
            | ProjectionElem::Subtype ( .. ) => {
                { ans.unsupport = true; break; }
            }
        }
    }
    ans
}

fn ownership_layout_to_rustbv(layout: &OwnershipLayout) -> RustBV {
    let mut v = Vec::default();
    for item in layout.iter() {
        match item {
            RawTypeOwner::Uninit => rap_error!("item of raw type owner is uninit"),
            RawTypeOwner::Unowned => v.push(false),
            RawTypeOwner::Owned => v.push(true),
        }
    }
    v
}

fn reverse_ownership_layout_to_rustbv(layout: &OwnershipLayout) -> RustBV {
    let mut v = Vec::default();
    for item in layout.iter() {
        match item {
            RawTypeOwner::Uninit => rap_error!("item of raw type owner is uninit"),
            RawTypeOwner::Unowned => v.push(true),
            RawTypeOwner::Owned => v.push(false),
        }
    }
    v
}

fn rustbv_merge(a:&RustBV, b: &RustBV) -> RustBV {
    assert_eq!(a.len(), b.len());
    let mut bv = Vec::new();
    for idx in 0..a.len() {
        bv.push(a[idx] || b[idx]);
    }
    bv
}

// Create an unsigned integer from bit bit-vector.
// The bit-vector has n bits
// the i'th bit (counting from 0 to n-1) is 1 if ans div 2^i mod 2 is 1.
fn rustbv_to_int(bv: &RustBV) -> u64 {
    let mut ans = 0;
    let mut base = 1;
    for tf in bv.iter() {
        ans = ans + base * (*tf as u64);
        base = base * 2;
    }
    ans
}

fn help_debug_goal_stmt<'tcx, 'ctx>(
    ctx: &'ctx z3::Context,
    goal: &'ctx z3::Goal<'ctx>,
    bidx: usize,
    sidx: usize,
) {
    let debug_name = format!("CONSTRAINTS: S {} {}", bidx, sidx);
    let dbg_bool = ast::Bool::new_const(ctx, debug_name);
    goal.assert(&dbg_bool);
}

fn help_debug_goal_term<'tcx, 'ctx>(
    ctx: &'ctx z3::Context,
    goal: &'ctx z3::Goal<'ctx>,
    bidx: usize,
) {
    let debug_name = format!("CONSTRAINTS: T {}", bidx);
    let dbg_bool = ast::Bool::new_const(ctx, debug_name);
    goal.assert(&dbg_bool);
}

type Disc = Option<VariantIdx>;
