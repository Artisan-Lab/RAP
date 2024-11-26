use crate::analysis::safedrop::graph::SafeDropGraph;
use crate::analysis::utils::show_mir::display_mir;
use crate::rap_warn;
use rustc_span::source_map::Spanned;
use rustc_span::Span;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use super::contracts::abstract_state::{
    AbstractState, AbstractStateItem, AlignState, StateType, VType, Value,
};
use super::contracts::contract::Contract;
use super::inter_record::{InterAnalysisRecord, GLOBAL_INTER_RECORDER};
use super::matcher::{get_arg_place, match_unsafe_api_and_check_contracts};
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use rustc_middle::{
    mir::{
        self, AggregateKind, BasicBlock, BasicBlockData, BinOp, CastKind, Operand, Place, Rvalue,
        Statement, StatementKind, Terminator, TerminatorKind,
    },
    ty::{self, GenericArgKind, Ty, TyKind},
};

//TODO: modify contracts vec to contract-bool pairs (we can also use path index to record path info)
pub struct CheckResult {
    pub func_name: String,
    pub func_span: Span,
    pub failed_contracts: Vec<(usize, Contract)>,
    pub passed_contracts: Vec<(usize, Contract)>,
}

impl CheckResult {
    pub fn new(func_name: &str, func_span: Span) -> Self {
        Self {
            func_name: func_name.to_string(),
            func_span,
            failed_contracts: Vec::new(),
            passed_contracts: Vec::new(),
        }
    }
}

pub struct BodyVisitor<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub def_id: DefId,
    pub safedrop_graph: SafeDropGraph<'tcx>,
    // abstract_states records the path index and variables' ab states in this path
    pub abstract_states: HashMap<usize, AbstractState>,
    pub unsafe_callee_report: HashMap<String, usize>,
    pub local_ty: HashMap<usize, (usize, usize)>,
    pub visit_time: usize,
    pub check_results: Vec<CheckResult>,
}

impl<'tcx> BodyVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, def_id: DefId, visit_time: usize) -> Self {
        let body = tcx.optimized_mir(def_id);
        Self {
            tcx,
            def_id,
            safedrop_graph: SafeDropGraph::new(body, tcx, def_id),
            abstract_states: HashMap::new(),
            unsafe_callee_report: HashMap::new(),
            local_ty: HashMap::new(),
            visit_time,
            check_results: Vec::new(),
        }
    }

    pub fn path_forward_check(&mut self) {
        let paths = self.get_all_paths();
        let body = self.tcx.optimized_mir(self.def_id);
        let locals = body.local_decls.clone();
        for (idx, local) in locals.iter().enumerate() {
            let local_ty = local.ty;
            let layout = self.visit_ty_and_get_layout(local_ty);
            self.local_ty.insert(idx, layout);
        }
        // display_mir(self.def_id,&body);
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
        // self.abstract_states_mop();
        // self.abstate_debug();
    }

    pub fn path_analyze_block(
        &mut self,
        block: &BasicBlockData<'tcx>,
        path_index: usize,
        bb_index: usize,
    ) {
        for statement in block.statements.iter() {
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
                unwind: _,
                call_source: _,
                fn_span,
            } => {
                let func_name = format!("{:?}", func);
                if let Operand::Constant(func_constant) = func {
                    if let ty::FnDef(ref callee_def_id, raw_list) = func_constant.const_.ty().kind()
                    {
                        if self.visit_time == 0 {
                            for generic_arg in raw_list.iter() {
                                match generic_arg.unpack() {
                                    GenericArgKind::Type(ty) => {
                                        if let Some(new_check_result) =
                                            match_unsafe_api_and_check_contracts(
                                                func_name.as_str(),
                                                args,
                                                &self.abstract_states.get(&path_index).unwrap(),
                                                *fn_span,
                                                ty,
                                            )
                                        {
                                            if let Some(_existing) =
                                                self.check_results.iter_mut().find(|result| {
                                                    result.func_name == new_check_result.func_name
                                                })
                                            {
                                            } else {
                                                self.check_results.push(new_check_result);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        self.handle_call(callee_def_id, args, path_index);
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
                    let rpjc_local = self
                        .safedrop_graph
                        .projection(self.tcx, true, rplace.clone());
                    if let Some(ab_state) = self.abstract_states.get(&path_index) {
                        if let Some(r_state_item) = ab_state.state_map.get(&rpjc_local) {
                            self.insert_path_abstate(path_index, lpjc_local, r_state_item.clone());
                        }
                    }
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
                self.insert_path_abstate(path_index, lpjc_local, Some(abitem));
            }
            Rvalue::Cast(cast_kind, op, ty) => match op {
                Operand::Move(rplace) | Operand::Copy(rplace) => {
                    let rpjc_local = self
                        .safedrop_graph
                        .projection(self.tcx, true, rplace.clone());
                    self.handle_cast(rpjc_local, lpjc_local, ty, path_index, cast_kind);
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
            Rvalue::Discriminant(_place) => {
                // println!("Discriminant {}:{:?}",lpjc_local,rvalue);
            }
            _ => {}
        }
    }

    pub fn handle_call(
        &mut self,
        def_id: &DefId,
        args: &Box<[Spanned<Operand>]>,
        path_index: usize,
    ) {
        if !self.tcx.is_mir_available(def_id) {
            return;
        }

        // get pre analysis state
        let mut pre_analysis_state = HashMap::new();
        for (idx, arg) in args.iter().enumerate() {
            let arg_place = get_arg_place(&arg.node);
            let ab_state_item = self.get_abstate_by_place_in_path(arg_place, path_index);
            pre_analysis_state.insert(idx, ab_state_item);
        }

        // check cache
        let mut recorder = GLOBAL_INTER_RECORDER.lock().unwrap();
        if let Some(record) = recorder.get_mut(def_id) {
            if record.is_pre_state_same(&pre_analysis_state) {
                // update directly
                self.update_post_state(&record.post_analysis_state, args, path_index);
                return;
            }
        }
        drop(recorder);

        // update post states and cache
        let mut inter_body_visitor: BodyVisitor<'_> =
            BodyVisitor::new(self.tcx, *def_id, self.visit_time + 1);
        inter_body_visitor.path_forward_check();
        let post_analysis_state: HashMap<usize, Option<AbstractStateItem>> =
            inter_body_visitor.get_args_post_states();
        // self.update_post_state(&post_analysis_state, args, path_index);
        let mut recorder = GLOBAL_INTER_RECORDER.lock().unwrap();
        recorder.insert(
            *def_id,
            InterAnalysisRecord::new(pre_analysis_state, post_analysis_state),
        );
        // drop(recorder);
    }

    // if inter analysis's params are in mut_ref, then we should update their post states
    pub fn update_post_state(
        &mut self,
        post_state: &HashMap<usize, Option<AbstractStateItem>>,
        args: &Box<[Spanned<Operand>]>,
        path_index: usize,
    ) {
        for (idx, arg) in args.iter().enumerate() {
            let arg_place = get_arg_place(&arg.node);
            if let Some(state_item) = post_state.get(&idx) {
                self.insert_path_abstate(path_index, arg_place, state_item.clone());
            }
        }
    }

    pub fn get_args_post_states(&mut self) -> HashMap<usize, Option<AbstractStateItem>> {
        let final_states = self.abstract_states_mop();
        let mut result_states = HashMap::new();
        let fn_sig = self.tcx.fn_sig(self.def_id).skip_binder();
        let num_params = fn_sig.inputs().skip_binder().len();
        for i in 0..num_params {
            if let Some(state) = final_states.state_map.get(&(i + 1)) {
                result_states.insert(i, state.clone());
            } else {
                result_states.insert(i, None);
            }
        }
        result_states
    }

    pub fn get_all_paths(&mut self) -> Vec<Vec<usize>> {
        self.safedrop_graph.solve_scc();
        let results = self.safedrop_graph.get_paths();
        results
    }

    pub fn abstract_states_mop(&mut self) -> AbstractState {
        let mut result_state = AbstractState {
            state_map: HashMap::new(),
        };

        for (_path_idx, abstract_state) in &self.abstract_states {
            for (var_index, state_item) in &abstract_state.state_map {
                if let Some(existing_state_item) = result_state.state_map.get_mut(&var_index) {
                    existing_state_item
                        .clone()
                        .unwrap()
                        .meet_state_item(&state_item.clone().unwrap());
                } else {
                    result_state
                        .state_map
                        .insert(*var_index, state_item.clone());
                }
            }
        }
        result_state
    }

    pub fn abstate_debug(&self) {
        if self.visit_time != 0 {
            return;
        }
        Self::display_hashmap(&self.local_ty, 1);
        display_mir(self.def_id, self.tcx.optimized_mir(self.def_id));
        println!("---------------");
        println!("--def_id: {:?}", self.def_id);

        let mut sorted_states: Vec<_> = self.abstract_states.iter().collect();
        sorted_states.sort_by(|a, b| a.0.cmp(b.0));
        for (path, abstract_state) in &sorted_states {
            println!("--Path-{:?}:", path);
            let mut sorted_state_map: Vec<_> = abstract_state.state_map.iter().collect();
            sorted_state_map.sort_by_key(|&(place, _)| place);
            for (place, ab_item) in sorted_state_map {
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
        abitem: Option<AbstractStateItem>,
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
        *self.local_ty.get(&place).unwrap()
    }

    pub fn visit_ty_and_get_layout(&self, ty: Ty<'tcx>) -> (usize, usize) {
        match ty.kind() {
            TyKind::RawPtr(ty, _)
            | TyKind::Ref(_, ty, _)
            | TyKind::Slice(ty)
            | TyKind::Array(ty, _) => self.get_layout_by_ty(*ty),
            _ => (0, 0),
        }
    }

    pub fn get_layout_by_ty(&self, ty: Ty<'tcx>) -> (usize, usize) {
        let param_env = self.tcx.param_env(self.def_id);
        if let Ok(_) = self.tcx.layout_of(param_env.and(ty)) {
            let layout = self.tcx.layout_of(param_env.and(ty)).unwrap();
            let align = layout.align.abi.bytes_usize();
            let size = layout.size.bytes() as usize;
            return (align, size);
        } else {
            match ty.kind() {
                TyKind::Array(inner_ty, _) | TyKind::Slice(inner_ty) => {
                    return self.get_layout_by_ty(*inner_ty);
                }
                _ => {}
            }
        }
        return (0, 0);
    }

    pub fn get_abstate_by_place_in_path(
        &self,
        place: usize,
        path_index: usize,
    ) -> Option<AbstractStateItem> {
        if let Some(abstate) = self.abstract_states.get(&path_index) {
            if let Some(_) = abstate.state_map.get(&place).cloned() {
                return abstate.state_map.get(&place).cloned().unwrap();
            }
        }
        return None;
    }

    pub fn display_hashmap<K, V>(map: &HashMap<K, V>, level: usize)
    where
        K: Ord + Debug + Hash,
        V: Debug,
    {
        let indent = "  ".repeat(level);
        let mut sorted_keys: Vec<_> = map.keys().collect();
        sorted_keys.sort();

        for key in sorted_keys {
            if let Some(value) = map.get(key) {
                println!("{}{:?}: {:?}", indent, key, value);
            }
        }
    }

    pub fn handle_cast(
        &mut self,
        rpjc_local: usize,
        lpjc_local: usize,
        ty: &Ty<'tcx>,
        path_index: usize,
        cast_kind: &CastKind,
    ) {
        let mut src_align = self.get_layout_by_place_usize(rpjc_local).0;
        match cast_kind {
            CastKind::PtrToPtr | CastKind::PointerCoercion(_, _) => {
                if let Some(r_abitem) = self.get_abstate_by_place_in_path(rpjc_local, path_index) {
                    for state in &r_abitem.state {
                        if let StateType::AlignState(r_align_state) = state {
                            match r_align_state {
                                AlignState::Small2BigCast(from, _to)
                                | AlignState::Big2SmallCast(from, _to) => {
                                    src_align = *from;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                let (dst_align, dst_size) = self.visit_ty_and_get_layout(*ty);
                let align_state = match dst_align.cmp(&src_align) {
                    std::cmp::Ordering::Greater => {
                        StateType::AlignState(AlignState::Small2BigCast(src_align, dst_align))
                    }
                    std::cmp::Ordering::Less => {
                        StateType::AlignState(AlignState::Big2SmallCast(src_align, dst_align))
                    }
                    std::cmp::Ordering::Equal => StateType::AlignState(AlignState::Aligned),
                };
                let abitem = AbstractStateItem::new(
                    (Value::None, Value::None),
                    VType::Pointer(dst_align, dst_size),
                    HashSet::from([align_state]),
                );
                self.insert_path_abstate(path_index, lpjc_local, Some(abitem));
            }
            _ => {}
        }
    }

    pub fn handle_binary_op(
        &mut self,
        first_op: &Operand,
        bin_op: &BinOp,
        second_op: &Operand,
        path_index: usize,
    ) {
        match bin_op {
            BinOp::Offset => {
                let first_place = self.handle_operand(first_op);
                let _second_place = self.handle_operand(second_op);
                let _abitem = self.get_abstate_by_place_in_path(first_place, path_index);
            }
            _ => {}
        }
    }

    pub fn handle_operand(&self, op: &Operand) -> usize {
        match op {
            Operand::Move(place) => place.local.as_usize(),
            Operand::Copy(place) => place.local.as_usize(),
            _ => 0,
        }
    }
}
