use crate::analysis::safedrop::graph::SafeDropGraph;
use crate::rap_warn;
use std::collections::{HashMap, HashSet};

use super::contracts::abstract_state::{
    AbstractState, AbstractStateItem, AlignState, StateType, VType, Value,
};
use super::inter_record::{InterAnalysisRecord, GLOBAL_INTER_RECORDER};
use super::matcher::match_unsafe_api_and_check_contracts;
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use rustc_middle::{
    mir::{
        self, AggregateKind, BasicBlock, BasicBlockData, Local, Operand, Place, Rvalue, Statement,
        StatementKind, Terminator, TerminatorKind,
    },
    ty::{self, GenericArgKind, Ty, TyKind},
};

pub struct BodyVisitor<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub def_id: DefId,
    pub safedrop_graph: SafeDropGraph<'tcx>,
    // abstract_states records the path index and variables' ab states in this path
    pub abstract_states: HashMap<usize, AbstractState>,
    pub unsafe_callee_report: HashMap<String, usize>,
    pub first_layer_flag: bool,
}

impl<'tcx> BodyVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, def_id: DefId, first_layer_flag: bool) -> Self {
        let body = tcx.optimized_mir(def_id);
        Self {
            tcx,
            def_id,
            safedrop_graph: SafeDropGraph::new(body, tcx, def_id),
            abstract_states: HashMap::new(),
            unsafe_callee_report: HashMap::new(),
            first_layer_flag,
        }
    }

    pub fn path_forward_check(&mut self) {
        let paths = self.get_all_paths();
        let body = self.tcx.optimized_mir(self.def_id);
        for (index, path_info) in paths.iter().enumerate() {
            self.abstract_states.insert(index, AbstractState::new());
            for block_index in path_info.iter() {
                if block_index >= &body.basic_blocks.len() {
                    continue;
                }
                self.path_analyze_block(
                    &body.basic_blocks[BasicBlock::from_usize(*block_index)].clone(),
                    index,
                    *block_index,
                );
                let tem_scc_sub_blocks = self.safedrop_graph.blocks[*block_index]
                    .scc_sub_blocks
                    .clone();
                // println!("father block {:?} scc sub blocks {:?}", block_index, tem_scc_sub_blocks);
                if tem_scc_sub_blocks.len() > 0 {
                    for sub_block in &tem_scc_sub_blocks {
                        self.path_analyze_block(
                            &body.basic_blocks[BasicBlock::from_usize(*sub_block)].clone(),
                            index,
                            *block_index,
                        );
                    }
                }
            }
        }
        self.abstract_states_mop();
    }

    pub fn path_analyze_block(
        &mut self,
        block: &BasicBlockData<'tcx>,
        path_index: usize,
        bb_index: usize,
    ) {
        for statement in block.statements.iter().rev() {
            self.path_analyze_statement(statement, path_index);
        }
        self.path_analyze_terminator(&block.terminator(), path_index, bb_index);
    }

    pub fn path_analyze_terminator(
        &mut self,
        terminator: &Terminator<'tcx>,
        path_index: usize,
        _bb_index: usize,
    ) {
        match &terminator.kind {
            TerminatorKind::Call {
                func,
                args,
                destination: _,
                target: _,
                ..
            } => {
                let func_name = format!("{:?}", func);
                if let Operand::Constant(func_constant) = func {
                    if let ty::FnDef(ref callee_def_id, raw_list) = func_constant.const_.ty().kind()
                    {
                        if self.first_layer_flag {
                            for generic_arg in raw_list.iter() {
                                match generic_arg.unpack() {
                                    GenericArgKind::Type(ty) => {
                                        match_unsafe_api_and_check_contracts(
                                            func_name.as_str(),
                                            args,
                                            &self.abstract_states.get(&path_index).unwrap(),
                                            ty,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                        //TODO:path_inter_analyze
                        self.handle_call(callee_def_id);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn path_analyze_statement(&mut self, statement: &Statement<'tcx>, _path_index: usize) {
        match statement.kind {
            StatementKind::Assign(box (ref lplace, ref rvalue)) => {
                self.path_analyze_assign(lplace, rvalue, _path_index);
            }
            StatementKind::Intrinsic(box ref intrinsic) => match intrinsic {
                mir::NonDivergingIntrinsic::CopyNonOverlapping(cno) => {
                    if cno.src.place().is_some() && cno.dst.place().is_some() {
                        let _src_pjc_local = self.safedrop_graph.projection(
                            self.tcx,
                            true,
                            cno.src.place().unwrap().clone(),
                        );
                        let _dst_pjc_local = self.safedrop_graph.projection(
                            self.tcx,
                            true,
                            cno.dst.place().unwrap().clone(),
                        );
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn path_analyze_assign(
        &mut self,
        lplace: &Place<'tcx>,
        rvalue: &Rvalue<'tcx>,
        path_index: usize,
    ) {
        let lpjc_local = self
            .safedrop_graph
            .projection(self.tcx, false, lplace.clone());
        match rvalue {
            Rvalue::Use(op) => match op {
                Operand::Move(rplace) | Operand::Copy(rplace) => {
                    let _rpjc_local =
                        self.safedrop_graph
                            .projection(self.tcx, true, rplace.clone());
                }
                _ => {}
            },
            Rvalue::Repeat(op, _const) => match op {
                Operand::Move(rplace) | Operand::Copy(rplace) => {
                    let _rpjc_local =
                        self.safedrop_graph
                            .projection(self.tcx, true, rplace.clone());
                }
                _ => {}
            },
            Rvalue::Ref(_, _, rplace) => {
                let rpjc_local = self
                    .safedrop_graph
                    .projection(self.tcx, true, rplace.clone());
                let (align, size) = self.get_layout_by_place_usize(rpjc_local);
                let abitem = AbstractStateItem::new(
                    (Value::None, Value::None),
                    VType::Pointer(align, size),
                    HashSet::from([StateType::AlignState(AlignState::Aligned)]),
                );
                self.insert_path_abstate(path_index, lpjc_local, abitem);
            }
            Rvalue::Cast(_cast_kind, op, ty) => match op {
                Operand::Move(rplace) | Operand::Copy(rplace) => {
                    let rpjc_local = self
                        .safedrop_graph
                        .projection(self.tcx, true, rplace.clone());
                    let (src_align, _src_size) = self.get_layout_by_place_usize(rpjc_local);
                    let (dst_align, dst_size) = self.visit_ty_and_get_layout(*ty);
                    let state = match dst_align.cmp(&src_align) {
                        std::cmp::Ordering::Greater => {
                            StateType::AlignState(AlignState::Small2BigCast)
                        }
                        std::cmp::Ordering::Less => {
                            StateType::AlignState(AlignState::Big2SmallCast)
                        }
                        std::cmp::Ordering::Equal => StateType::AlignState(AlignState::Aligned),
                    };
                    let abitem = AbstractStateItem::new(
                        (Value::None, Value::None),
                        VType::Pointer(dst_align, dst_size),
                        HashSet::from([state]),
                    );
                    self.insert_path_abstate(path_index, lpjc_local, abitem);
                }
                _ => {}
            },
            Rvalue::BinaryOp(_bin_op, box (ref _op1, ref _op2)) => {}
            Rvalue::ShallowInitBox(op, _ty) => match op {
                Operand::Move(rplace) | Operand::Copy(rplace) => {
                    let _rpjc_local =
                        self.safedrop_graph
                            .projection(self.tcx, true, rplace.clone());
                }
                _ => {}
            },
            Rvalue::Aggregate(box ref agg_kind, _op_vec) => match agg_kind {
                AggregateKind::Array(_ty) => {}
                _ => {}
            },
            // Rvalue::Discriminant(_place) => {
            //     println!("{}:{:?}",llocal,rvalue);
            // }
            _ => {}
        }
    }

    pub fn handle_call(&mut self, def_id: &DefId) {
        let pre_analysis_state = HashMap::new();
        let mut recorder = GLOBAL_INTER_RECORDER.lock().unwrap();
        if let Some(record) = recorder.get_mut(def_id) {
            if record.is_pre_state_same(&pre_analysis_state) {
                // update directly
                self.update_inter_state_directly();
                return;
            }
        }
        let _inter_body_visitor = BodyVisitor::new(self.tcx, *def_id, false).path_forward_check();
        let post_analysis_state = HashMap::new();
        recorder.insert(
            *def_id,
            InterAnalysisRecord::new(pre_analysis_state, post_analysis_state),
        );
    }

    pub fn update_inter_state_directly(&mut self) {}

    pub fn visit_ty_and_get_layout(&self, ty: Ty<'tcx>) -> (usize, usize) {
        match ty.kind() {
            TyKind::RawPtr(ty, _) | TyKind::Ref(_, ty, _) | TyKind::Slice(ty) => {
                self.get_layout_by_ty(*ty)
            }
            _ => (0, 0),
        }
    }

    pub fn get_all_paths(&mut self) -> Vec<Vec<usize>> {
        self.safedrop_graph.solve_scc();
        let results = self.safedrop_graph.get_paths();
        results
    }

    pub fn abstract_states_mop(&mut self) {
        let mut result_state = AbstractState {
            state_map: HashMap::new(),
        };

        for (_path_idx, abstract_state) in &self.abstract_states {
            for (var_index, state_item) in &abstract_state.state_map {
                if let Some(existing_state_item) = result_state.state_map.get_mut(&var_index) {
                    existing_state_item.meet_state_item(state_item);
                } else {
                    result_state
                        .state_map
                        .insert(*var_index, state_item.clone());
                }
            }
        }
    }

    pub fn abstate_debug(&self) {
        for (path, abstract_state) in &self.abstract_states {
            println!("Path-{:?}:", path);
            for (place, ab_item) in &abstract_state.state_map {
                println!("Place-{:?} has abstract states:{:?}", place, ab_item);
            }
        }
    }

    pub fn get_all_callees(&self, def_id: DefId) -> Vec<String> {
        let mut results = Vec::new();
        let body = self.tcx.optimized_mir(def_id);
        let bb_len = body.basic_blocks.len();
        for i in 0..bb_len {
            let callees = Self::get_terminator_callee(
                body.basic_blocks[BasicBlock::from_usize(i)]
                    .clone()
                    .terminator(),
            );
            results.extend(callees);
        }
        results
    }

    pub fn get_terminator_callee(terminator: &Terminator<'tcx>) -> Vec<String> {
        let mut results = Vec::new();
        match &terminator.kind {
            TerminatorKind::Call { func, .. } => {
                let func_name = format!("{:?}", func);
                results.push(func_name);
            }
            _ => {}
        }
        results
    }

    pub fn update_callee_report_level(&mut self, unsafe_callee: String, report_level: usize) {
        self.unsafe_callee_report
            .entry(unsafe_callee)
            .and_modify(|e| {
                if report_level < *e {
                    *e = report_level;
                }
            })
            .or_insert(report_level);
    }

    // level: 0 bug_level, 1-3 unsound_level
    // TODO: add more information about the result
    pub fn output_results(&self, threshold: usize) {
        for (unsafe_callee, report_level) in &self.unsafe_callee_report {
            if *report_level == 0 {
                rap_warn!("Find one bug in {:?}!", unsafe_callee);
            } else if *report_level <= threshold {
                rap_warn!("Find an unsoundness issue in {:?}!", unsafe_callee);
            }
        }
    }

    pub fn insert_path_abstate(
        &mut self,
        path_index: usize,
        place: usize,
        abitem: AbstractStateItem,
    ) {
        self.abstract_states
            .entry(path_index)
            .or_insert_with(|| AbstractState {
                state_map: HashMap::new(),
            })
            .state_map
            .insert(place, abitem);
    }

    pub fn get_layout_by_place_usize(&self, place: usize) -> (usize, usize) {
        let local_place = Place::from(Local::from_usize(place));
        let body = self.tcx.optimized_mir(self.def_id);
        let place_ty = local_place.ty(body, self.tcx).ty;
        self.visit_ty_and_get_layout(place_ty)
    }

    pub fn get_layout_by_ty(&self, ty: Ty<'tcx>) -> (usize, usize) {
        let param_env = self.tcx.param_env(self.def_id);
        let layout = self.tcx.layout_of(param_env.and(ty)).unwrap();
        let align = layout.align.abi.bytes_usize();
        let size = layout.size.bytes() as usize;
        (align, size)
    }

    pub fn get_abstate_by_place(&self) {}
}
