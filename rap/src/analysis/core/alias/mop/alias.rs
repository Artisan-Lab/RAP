use rustc_middle::ty;
use rustc_middle::mir::{TerminatorKind, Operand, Place, ProjectionElem};
use std::fmt;
use crate::rap_error;
use super::graph::*;
use super::types::*;
use super::mop::*;
use super::super::mop::FnMap;

impl<'tcx> MopGraph<'tcx> {
    /* alias analysis for a single block */
    pub fn alias_bb(&mut self, bb_index: usize) {
        for stmt in self.blocks[bb_index].const_value.clone() {
            self.constant.insert(stmt.0, stmt.1);
        }
        let cur_block = self.blocks[bb_index].clone();
        for assign in cur_block.assignments {
            let mut lv_aliaset_idx = self.projection(false, assign.lv.clone());
            let rv_aliaset_idx = self.projection(true, assign.rv.clone());
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
            if self.values[lv_aliaset_idx].local != self.values[rv_aliaset_idx].local {
                self.merge_alias(lv_aliaset_idx, rv_aliaset_idx);
            }
        }        
    }

    /* Check the aliases introduced by the terminators (function call) of a scc block */
    pub fn alias_bbcall(&mut self, bb_index: usize, fn_map: &mut FnMap){
        let cur_block = self.blocks[bb_index].clone();
        for call in cur_block.calls {
            if let TerminatorKind::Call { ref func, ref args, ref destination, target:_, unwind: _, call_source: _, fn_span: _ } = call.kind {
                if let Operand::Constant(ref constant) = func {
                    let lv = self.projection(false, destination.clone());
                    let mut merge_vec = Vec::new();
                    merge_vec.push(lv);
                    let mut may_drop_flag = 0;
                    if self.values[lv].may_drop {
                        may_drop_flag += 1;
                    }
                    for arg in args {
                        match arg {
                            Operand::Copy(ref p) => {
                                let rv = self.projection(true, p.clone());
                                merge_vec.push(rv);
                                if self.values[rv].may_drop {
                                    may_drop_flag += 1;
                                }
                            },
                            Operand::Move(ref p) => {
                                let rv = self.projection(true, p.clone());
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
                        //if may_drop_flag > 1 || Self::should_check(target_id.clone()) == false {
                        if may_drop_flag > 1 {
                            if self.tcx.is_mir_available(*target_id) {
                                if fn_map.contains_key(&target_id) {
                                    let assignments = fn_map.get(&target_id).unwrap();
                                    for assign in assignments.alias_vec.iter() {
                                        if !assign.valuable() {
                                            continue;
                                        }
                                        self.merge(assign, &merge_vec);
                                    }
                                }
                                else{
                                    if fn_map.contains_key(&target_id) {
                                        continue;
                                    }
                                    let mut mop_graph = MopGraph::new(self.tcx, *target_id);
                                    mop_graph.solve_scc();
                                    mop_graph.check(0, fn_map);
                                    let ret_alias = mop_graph.ret_alias.clone();
                                    for assign in ret_alias.alias_vec.iter() {
                                        if !assign.valuable(){
                                            continue;
                                        }
                                        self.merge(assign, &merge_vec);
                                    }
                                    fn_map.insert(*target_id, ret_alias);
                                }
                            }
                            else {
                                if self.values[lv].may_drop {
                                    if target_id.index.as_usize() == CALL_MUT 
                                        || target_id.index.as_usize() == NEXT {
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

    /*
     * This is the function for field sensitivity
     * If the projection is a deref, we directly return its head alias or alias[0].
     * If the id is not a ref, we further make the id and its first element an alias, i.e., level-insensitive
     *
     */
    pub fn projection(&mut self, is_right: bool, place: Place<'tcx>) -> usize {
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
                        let param_env = self.tcx.param_env(self.def_id);
                        let need_drop = ty.needs_drop(self.tcx, param_env);
                        let may_drop = !is_not_drop(self.tcx, ty);
                        let mut node = ValueNode::new(new_id, local, need_drop, need_drop || may_drop);
                        node.kind = kind(ty);
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
        for index in ret_alias.left_field_seq.iter() {
            if self.values[lv].fields.contains_key(&index) == false {
                let need_drop = ret_alias.left_need_drop;
                let may_drop = ret_alias.left_may_drop;
                let mut node = ValueNode::new(self.values.len(), left_init, need_drop, may_drop);
                node.kind = TyKind::RawPtr;
                node.field_id = *index;
                self.values[lv].fields.insert(*index, node.index);
                self.values.push(node);
            }
            lv = *self.values[lv].fields.get(&index).unwrap();
        }
        for index in ret_alias.right_field_seq.iter() {
            if self.values[rv].alias[0] != rv {
                rv = self.values[rv].alias[0];
                right_init = self.values[rv].local;
            }
            if self.values[rv].fields.contains_key(&index) == false {
                let need_drop = ret_alias.right_need_drop;
                let may_drop = ret_alias.right_may_drop;
                let mut node = ValueNode::new(self.values.len(), right_init, need_drop, may_drop);
                node.kind = TyKind::RawPtr;
                node.field_id = *index;
                self.values[rv].fields.insert(*index, node.index);
                self.values.push(node);
            }
            rv = *self.values[rv].fields.get(&index).unwrap();
        }
        self.merge_alias(lv, rv);
    }

    //merge the result of current path to the final result.
    pub fn merge_results(&mut self, results_nodes: Vec<ValueNode>) {
        for node in results_nodes.iter() {
            if node.local <= self.arg_size {
                if node.alias[0] != node.index || node.alias.len() > 1 {
                    for alias in node.alias.clone() {
                        if results_nodes[alias].local <= self.arg_size
                        && alias != node.index
                        && node.local != results_nodes[alias].local {
                            let left_node = node;
                            let right_node = &results_nodes[alias];
                            let mut new_alias = RetAlias::new( 
                                left_node.local, left_node.may_drop, left_node.need_drop,
                                right_node.local, right_node.may_drop, right_node.need_drop
			                );
                            new_alias.left_field_seq = self.get_field_seq(left_node); 
                            new_alias.right_field_seq = self.get_field_seq(right_node); 
                            self.ret_alias.alias_vec.push(new_alias);
                        }
                    }
                }
            }
        }
    }

    pub fn get_field_seq(&self, value: &ValueNode)-> Vec<usize> { 
        let mut field_id_seq = vec![];
        let mut node_ref = value;
        while node_ref.field_id != usize::MAX {
            field_id_seq.push(node_ref.field_id);
            node_ref = &self.values[value.father]; 
        }
        return field_id_seq;
    }
}
/*
 * To store the alias relationships among arguments and return values.
 */
#[derive(Debug,Clone)]
pub struct RetAlias{
    pub left_index: usize,
    pub left_field_seq: Vec<usize>, 
    pub left_may_drop: bool, 
    pub left_need_drop: bool,
    pub right_index: usize,
    pub right_field_seq: Vec<usize>,
    pub right_may_drop: bool, 
    pub right_need_drop: bool,
}

impl RetAlias{
    pub fn new(left_index: usize, left_may_drop: bool, left_need_drop: bool,
        right_index: usize, right_may_drop: bool, right_need_drop: bool) -> RetAlias{
        RetAlias {
            left_index: left_index,
            left_field_seq: Vec::<usize>::new(),
            left_may_drop: left_may_drop,
            left_need_drop: left_need_drop,
            right_index: right_index,
            right_field_seq: Vec::<usize>::new(),
            right_may_drop: right_may_drop,
            right_need_drop: right_need_drop,
        }
    }

    pub fn valuable(&self) -> bool{
        return self.left_may_drop && self.right_may_drop;
    }
}

impl fmt::Display for RetAlias {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
            "left_index: {} = right_index: {}",
            self.left_index, self.right_index
        )
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
}

impl FnRetAlias {
    pub fn new(arg_size: usize) -> FnRetAlias{
        Self { 
            arg_size: arg_size, 
            alias_vec: Vec::<RetAlias>::new(),
        }
    }
}

impl fmt::Display for FnRetAlias {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
             "  alias_vec:[{}]",
            self.alias_vec.iter()
                .map(|alias| format!("{},", alias))  // Indentation for nested struct display
                .collect::<Vec<String>>()
                .join(",\n")  // Join the strings with a comma and newline
        )
    }
}
