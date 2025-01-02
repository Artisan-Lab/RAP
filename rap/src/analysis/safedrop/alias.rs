use rustc_middle::mir::{Operand, Place, ProjectionElem, TerminatorKind};
use rustc_middle::ty;
use rustc_middle::ty::TyCtxt;

use super::graph::*;
use super::types::*;
use crate::analysis::core::alias::{FnMap, RetAlias};
use crate::rap_error;

impl<'tcx> SafeDropGraph<'tcx> {
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
                    self.alias_set[lv_aliaset_idx] = rv_aliaset_idx;
                    continue;
                }
                AssignType::InitBox => {
                    lv_aliaset_idx = *self.values[lv_aliaset_idx].fields.get(&0).unwrap();
                }
                _ => {} // Copy or Move
            }
            self.uaf_check(
                rv_aliaset_idx,
                assign.span,
                assign.rv.local.as_usize(),
                false,
            );
            self.fill_birth(lv_aliaset_idx, self.scc_indices[bb_index] as isize);
            if self.values[lv_aliaset_idx].local != self.values[rv_aliaset_idx].local {
                self.merge_alias(lv_aliaset_idx, rv_aliaset_idx);
            }
        }
    }

    /* Check the aliases introduced by the terminators (function call) of a scc block */
    pub fn alias_bbcall(&mut self, bb_index: usize, tcx: TyCtxt<'tcx>, fn_map: &FnMap) {
        let cur_block = self.blocks[bb_index].clone();
        for call in cur_block.calls {
            if let TerminatorKind::Call {
                ref func,
                ref args,
                ref destination,
                target: _,
                unwind: _,
                call_source: _,
                fn_span: _,
            } = call.kind
            {
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
                        match arg.node {
                            Operand::Copy(ref p) => {
                                let rv = self.projection(tcx, true, p.clone());
                                self.uaf_check(rv, call.source_info.span, p.local.as_usize(), true);
                                merge_vec.push(rv);
                                if self.values[rv].may_drop {
                                    may_drop_flag += 1;
                                }
                            }
                            Operand::Move(ref p) => {
                                let rv = self.projection(tcx, true, p.clone());
                                self.uaf_check(rv, call.source_info.span, p.local.as_usize(), true);
                                merge_vec.push(rv);
                                if self.values[rv].may_drop {
                                    may_drop_flag += 1;
                                }
                            }
                            Operand::Constant(_) => {
                                merge_vec.push(0);
                            }
                        }
                    }
                    if let ty::FnDef(ref target_id, _) = constant.const_.ty().kind() {
                        if may_drop_flag > 1 {
                            if tcx.is_mir_available(*target_id) {
                                if fn_map.contains_key(&target_id) {
                                    let assignments = fn_map.get(&target_id).unwrap();
                                    for assign in assignments.aliases().iter() {
                                        if !assign.valuable() {
                                            continue;
                                        }
                                        self.merge(assign, &merge_vec);
                                    }
                                }
                            } else {
                                if self.values[lv].may_drop {
                                    if self.corner_handle(lv, &merge_vec, *target_id) {
                                        continue;
                                    }
                                    let mut right_set = Vec::new();
                                    for rv in &merge_vec {
                                        if self.values[*rv].may_drop
                                            && lv != *rv
                                            && self.values[lv].is_ptr()
                                        {
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
        for i in 0..self.alias_set.len() {
            if self.union_is_same(i, node) && self.values[i].birth == -1 {
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
                    //proj_id = self.values[proj_id].alias[0];
                    proj_id = self.alias_set[proj_id];
                }
                /*
                 * Objective: 2 = 1.0; 0 = 2.0; => 0 = 1.0.0
                 */
                ProjectionElem::Field(field, ty) => {
                    if is_right && self.alias_set[proj_id] != proj_id {
                        proj_id = self.alias_set[proj_id];
                        local = self.values[proj_id].local;
                    }
                    let field_idx = field.as_usize();
                    if !self.values[proj_id].fields.contains_key(&field_idx) {
                        let param_env = tcx.param_env(self.def_id);
                        let need_drop = ty.needs_drop(tcx, param_env);
                        let may_drop = !is_not_drop(tcx, ty);
                        let mut node =
                            ValueNode::new(new_id, local, need_drop, need_drop || may_drop);
                        node.kind = kind(ty);
                        node.birth = self.values[proj_id].birth;
                        node.field_id = field_idx;
                        self.values[proj_id].fields.insert(field_idx, node.index);
                        self.alias_set.push(self.values.len());
                        self.dead_record.push(false);
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
        // if self.values[lv].alias.len() > 1 {
        //     let mut alias_clone = self.values[rv].alias.clone();
        //     self.values[lv].alias.append(&mut alias_clone);
        // } else {
        //     self.values[lv].alias = self.values[rv].alias.clone();
        // }
        self.union_merge(lv, rv);

        for field in self.values[rv].fields.clone().into_iter() {
            if !self.values[lv].fields.contains_key(&field.0) {
                let mut node = ValueNode::new(
                    self.values.len(),
                    self.values[lv].local,
                    self.values[field.1].need_drop,
                    self.values[field.1].may_drop,
                );
                node.kind = self.values[field.1].kind;
                node.birth = self.values[lv].birth;
                node.field_id = field.0;
                self.values[lv].fields.insert(field.0, node.index);
                self.alias_set.push(self.values.len());
                self.dead_record.push(false);
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
        for index in ret_alias.left_field_seq.iter() {
            if self.values[lv].fields.contains_key(&index) == false {
                let need_drop = ret_alias.left_need_drop;
                let may_drop = ret_alias.left_may_drop;
                let mut node = ValueNode::new(self.values.len(), left_init, need_drop, may_drop);
                node.kind = TyKind::RawPtr;
                node.birth = self.values[lv].birth;
                node.field_id = *index;
                self.values[lv].fields.insert(*index, node.index);
                self.alias_set.push(self.values.len());
                self.dead_record.push(false);
                self.values.push(node);
            }
            lv = *self.values[lv].fields.get(&index).unwrap();
        }
        for index in ret_alias.right_field_seq.iter() {
            // if self.values[rv].alias[0] != rv {
            if self.union_is_same(rv, self.alias_set[rv]) {
                rv = self.values[rv].index;
                right_init = self.values[rv].local;
            }
            if !self.values[rv].fields.contains_key(&index) {
                let need_drop = ret_alias.right_need_drop;
                let may_drop = ret_alias.right_may_drop;
                let mut node = ValueNode::new(self.values.len(), right_init, need_drop, may_drop);
                node.kind = TyKind::RawPtr;
                node.birth = self.values[rv].birth;
                node.field_id = *index;
                self.values[rv].fields.insert(*index, node.index);
                self.alias_set.push(self.values.len());
                self.dead_record.push(false);
                self.values.push(node);
            }
            rv = *self.values[rv].fields.get(&index).unwrap();
        }
        self.merge_alias(lv, rv);
    }

    #[inline]
    pub fn union_find(&mut self, e: usize) -> usize {
        let mut r = e;
        while self.alias_set[r] != r {
            r = self.alias_set[r];
        }
        r
    }

    #[inline]
    pub fn union_merge(&mut self, e1: usize, e2: usize) {
        let f1 = self.union_find(e1);
        let f2 = self.union_find(e2);

        if f1 < f2 {
            self.alias_set[f2] = f1;
        }
        if f1 > f2 {
            self.alias_set[f1] = f2;
        }

        for member in 0..self.alias_set.len() {
            self.alias_set[member] = self.union_find(self.alias_set[member]);
        }
    }

    #[inline]
    pub fn union_is_same(&mut self, e1: usize, e2: usize) -> bool {
        let f1 = self.union_find(e1);
        let f2 = self.union_find(e2);
        f1 == f2
    }

    pub fn union_has_alias(&mut self, e: usize) -> bool {
        for i in 0..self.alias_set.len() {
            if i == e {
                continue;
            }
            if self.union_is_same(e, i) {
                return true;
            }
        }
        false
    }
}
