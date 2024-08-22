use rustc_middle::ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::mir::{TerminatorKind, Operand, Place, ProjectionElem};
use rustc_data_structures::fx::FxHashSet;

use crate::rap_error;
use super::graph::*;
use super::types::*;
use super::log::*;
use super::safedrop::*;
use log::Log;

impl<'tcx> SafeDropGraph<'tcx>{
    /* alias analysis for a single block */
    pub fn alias_bb(&mut self, bb_index: usize, tcx: TyCtxt<'tcx>) {
        for stmt in self.blocks[bb_index].const_value.clone() {
            self.constant.insert(stmt.0, stmt.1);
        }
        let cur_block = self.blocks[bb_index].clone();
        for assign in cur_block.assignments {
            let mut lv_aliaset_idx = self.projection(tcx, false, assign.lv.clone());
            let rv_aliaset_idx = self.projection(tcx, true, assign.rv.clone());
            match assign.atype {
                AssignType::Variant => {
                    self.values[lv_aliaset_idx].alias[0] = rv_aliaset_idx;
                    continue;
                },
                AssignType::InitBox => {
                    lv_aliaset_idx = *self.values[lv_aliaset_idx].fields.get(&0).unwrap();
                },
                _ => { }, // Copy or Move
            }
            self.uaf_check(rv_aliaset_idx, assign.span, assign.rv.local.as_usize(), false);
            self.fill_birth(lv_aliaset_idx, self.scc_indices[bb_index] as isize);
            if self.values[lv_aliaset_idx].local != self.values[rv_aliaset_idx].local {
                self.merge_alias(lv_aliaset_idx, rv_aliaset_idx);
            }
        }        
    }

    /* Check the aliases introduced by the terminators (function call) of a scc block */
    pub fn alias_bbcall(&mut self, bb_index: usize, tcx: TyCtxt<'tcx>, func_map: &mut FuncMap){
        let cur_block = self.blocks[bb_index].clone();
        for call in cur_block.calls {
            if let TerminatorKind::Call { ref func, ref args, ref destination, target:_, unwind: _, call_source: _, fn_span: _ } = call.kind {
                if let Operand::Constant(ref constant) = func {
                    let lv = self.projection(tcx, false, destination.clone());
                    self.values[lv].birth = self.scc_indices[bb_index] as isize;
                    let mut merge_vec = Vec::new();
                    merge_vec.push(lv);
                    let mut may_drop_flag = 0;
                    if self.values[lv].may_drop {
                        may_drop_flag += 1;
                    }
                    for arg in args {
                        match arg {
                            Operand::Copy(ref p) => {
                                let rv = self.projection(tcx, true, p.clone());
                                self.uaf_check(rv, call.source_info.span, p.local.as_usize(), true);
                                merge_vec.push(rv);
                                if self.values[rv].may_drop {
                                    may_drop_flag += 1;
                                }
                            },
                            Operand::Move(ref p) => {
                                let rv = self.projection(tcx, true, p.clone());
                                self.uaf_check(rv, call.source_info.span, p.local.as_usize(), true);
                                merge_vec.push(rv);
                                if self.values[rv].may_drop {
                                    may_drop_flag += 1;
                                }
                            },
                            Operand::Constant(_) => {
                                merge_vec.push(0);
                            },
                        }
                    }
                    if let ty::FnDef(ref target_id, _) = constant.const_.ty().kind() {
                        if may_drop_flag > 1 || (may_drop_flag > 0 && Self::should_check(target_id.clone()) == false) {
                            if tcx.is_mir_available(*target_id) {
                                if func_map.map.contains_key(&target_id.index.as_usize()) {
                                    let assignments = func_map.map.get(&target_id.index.as_usize()).unwrap();
                                    for assign in assignments.alias_vec.iter() {
                                        if !assign.valuable() {
                                            continue;
                                        }
                                        self.merge(assign, &merge_vec);
                                    }
                                    for dead in assignments.dead.iter() {
                                        let drop = merge_vec[*dead];
                                        self.dead_node(drop, 99999, &call.source_info, false);
                                    }
                                }
                                else{
                                    if func_map.set.contains(&target_id.index.as_usize()) {
                                        continue;
                                    }
                                    func_map.set.insert(target_id.index.as_usize());
                                    let func_body = tcx.optimized_mir(*target_id);
                                    let mut safedrop_graph = SafeDropGraph::new(&func_body, tcx, *target_id);
                                    safedrop_graph.solve_scc();
                                    safedrop_graph.check(0, tcx, func_map);
                                    let ret_alias = safedrop_graph.ret_alias.clone();
                                    for assign in ret_alias.alias_vec.iter() {
                                        if !assign.valuable(){
                                            continue;
                                        }
                                        self.merge(assign, &merge_vec);
                                    }
                                    for dead in ret_alias.dead.iter() {
                                        let drop = merge_vec[*dead];
                                        self.dead_node(drop, 99999, &call.source_info, false);
                                    }
                                    func_map.map.insert(target_id.index.as_usize(), ret_alias);
                                }
                            }
                            else {
                                if self.values[lv].may_drop {
                                    if self.corner_handle(lv, &merge_vec, *target_id){
                                        continue;
                                    }
                                    let mut right_set = Vec::new(); 
                                    for rv in &merge_vec {
                                        if self.values[*rv].may_drop && lv != *rv && self.values[lv].is_ptr(){
                                            right_set.push(*rv);
                                        }
                                    }
                                    if right_set.len() == 1 {
                                        self.merge_alias(lv, right_set[0]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // assign to the variable _x, we will set the birth of _x and its child self.values a new birth.
    pub fn fill_birth(&mut self, node: usize, birth: isize) {
        self.values[node].birth = birth;
        //TODO: check the correctness.
        for i in self.values[node].alias.clone() {
            if self.values[i].birth == -1 {
                self.values[i].birth = birth;
            }
        }
        for i in self.values[node].fields.clone().into_iter() {
            self.fill_birth(i.1, birth); //i.1 corresponds to the local field.
        }
    }

    /*
     * This is the function for field sensitivity
     * If the projection is a deref, we directly return its head alias or alias[0].
     * If the id is not a ref, we further make the id and its first element an alias, i.e., level-insensitive
     *
     */
    pub fn projection(&mut self, tcx: TyCtxt<'tcx>, is_right: bool, place: Place<'tcx>) -> usize {
        let mut local = place.local.as_usize();
        let mut proj_id = local;
        for proj in place.projection {
            let new_id = self.values.len();
            match proj {
                ProjectionElem::Deref => {
                    proj_id = self.values[proj_id].alias[0];
                }
                /*
                 * Objective: 2 = 1.0; 0 = 2.0; => 0 = 1.0.0
                 */
                ProjectionElem::Field(field, ty) => {
                    if is_right && self.values[proj_id].alias[0] != proj_id {
                        proj_id = self.values[proj_id].alias[0];
                        local = self.values[proj_id].local;
                    }
                    let field_idx = field.as_usize();
                    if !self.values[proj_id].fields.contains_key(&field_idx) {
                        let param_env = tcx.param_env(self.def_id);
                        let need_drop = ty.needs_drop(tcx, param_env);
                        let may_drop = !is_not_drop(tcx, ty);
                        let mut node = ValueNode::new(new_id, local, need_drop, need_drop || may_drop);
                        node.kind = kind(ty);
                        node.birth = self.values[proj_id].birth;
                        node.field_id = field_idx;
                        self.values[proj_id].fields.insert(field_idx, node.index);
                        self.values.push(node);
                    }
                    proj_id = *self.values[proj_id].fields.get(&field_idx).unwrap();
                }
                _ => {}
            }
        }
        return proj_id;
    }

    //instruction to assign alias for a variable.
    pub fn merge_alias(&mut self, lv: usize, rv: usize) {
        if self.values[lv].alias.len() > 1 {
            let mut alias_clone = self.values[rv].alias.clone();
            self.values[lv].alias.append(&mut alias_clone);
        } else {
            self.values[lv].alias = self.values[rv].alias.clone();
        }
        for field in self.values[rv].fields.clone().into_iter(){
            if !self.values[lv].fields.contains_key(&field.0) {
                let mut node = ValueNode::new(self.values.len(), self.values[lv].local, self.values[field.1].need_drop, self.values[field.1].may_drop);
                node.kind = self.values[field.1].kind;
                node.birth = self.values[lv].birth;
                node.field_id = field.0;
                self.values[lv].fields.insert(field.0, node.index);
                self.values.push(node);
            }
            let lv_field = *(self.values[lv].fields.get(&field.0).unwrap());
            self.merge_alias(lv_field, field.1);
        }
    }
    //inter-procedure instruction to merge alias.
    pub fn merge(&mut self, ret_alias: &RetAlias, arg_vec: &Vec<usize>) {
        if ret_alias.left_index >= arg_vec.len() || ret_alias.right_index >= arg_vec.len() {
            rap_error!("Vector error!");
            return;
        }
        let left_init = arg_vec[ret_alias.left_index];
        let mut right_init = arg_vec[ret_alias.right_index];
        let mut lv = left_init;
        let mut rv = right_init;
        for index in ret_alias.left.iter() {
            if self.values[lv].fields.contains_key(&index) == false {
                let need_drop = ret_alias.left_need_drop;
                let may_drop = ret_alias.left_may_drop;
                let mut node = ValueNode::new(self.values.len(), left_init, need_drop, may_drop);
                node.kind = TyKind::RawPtr;
                node.birth = self.values[lv].birth;
                node.field_id = *index;
                self.values[lv].fields.insert(*index, node.index);
                self.values.push(node);
            }
            lv = *self.values[lv].fields.get(&index).unwrap();
        }
        for index in ret_alias.right.iter() {
            if self.values[rv].alias[0] != rv {
                rv = self.values[rv].alias[0];
                right_init = self.values[rv].local;
            }
            if self.values[rv].fields.contains_key(&index) == false {
                let need_drop = ret_alias.right_need_drop;
                let may_drop = ret_alias.right_may_drop;
                let mut node = ValueNode::new(self.values.len(), right_init, need_drop, may_drop);
                node.kind = TyKind::RawPtr;
                node.birth = self.values[rv].birth;
                node.field_id = *index;
                self.values[rv].fields.insert(*index, node.index);
                self.values.push(node);
            }
            rv = *self.values[rv].fields.get(&index).unwrap();
        }
        self.merge_alias(lv, rv);
    }
}
/*
 * To store the alias relationships among arguments and return values.
 */
#[derive(Debug,Clone)]
pub struct RetAlias{
    pub left_index: usize,
    pub left: Vec<usize>, //field
    pub left_may_drop: bool, 
    pub left_need_drop: bool,
    pub right_index: usize,
    pub right: Vec<usize>,
    pub right_may_drop: bool, 
    pub right_need_drop: bool,
    pub atype: usize,
}

impl RetAlias{
    pub fn new(atype: usize, left_index: usize, left_may_drop: bool, left_need_drop: bool,
        right_index: usize, right_may_drop: bool, right_need_drop: bool) -> RetAlias{
        let left = Vec::<usize>::new();
        let right = Vec::<usize>::new();
        RetAlias{
            left_index: left_index,
            left: left,
            left_may_drop: left_may_drop,
            left_need_drop: left_need_drop,
            right_index: right_index,
            right: right,
            right_may_drop: right_may_drop,
            right_need_drop: right_need_drop,
            atype: atype
        }
    }

    pub fn valuable(&self) -> bool{
        return self.left_may_drop && self.right_may_drop;
    }
}

/*
 * To store the alias relationships among arguments and return values.
 * Each function may have multiple return instructions, leading to different RetAlias.
 */
#[derive(Debug, Clone)]
pub struct FnRetAlias {
    pub arg_size: usize,
    pub alias_vec: Vec<RetAlias>,
    pub dead: FxHashSet<usize>,
}

impl FnRetAlias {
    pub fn new(arg_size: usize) -> FnRetAlias{
        let alias_vec = Vec::<RetAlias>::new();
        let dead = FxHashSet::default();
        FnRetAlias { arg_size: arg_size, alias_vec: alias_vec, dead: dead }
    }
}


