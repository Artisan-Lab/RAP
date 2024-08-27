use rustc_middle::mir::SourceInfo;
use rustc_span::Span;
use rustc_span::symbol::Symbol;
use rustc_data_structures::fx::FxHashSet;
use crate::utils::utils::*;
use super::graph::*;
use super::alias::*;

impl<'tcx> SafeDropGraph<'tcx> {
    pub fn report_bugs(&self) {
	    let filename = get_filename(self.tcx, self.def_id);
        match filename {
	        Some(filename) => { if filename.contains(".cargo") { return; } },
            None => {},
        }
        if self.bug_records.is_bug_free(){
            return;
        }
        let fn_name = match get_fn_name(self.tcx, self.def_id) {
            Some(name) => name,
            None => Symbol::intern("no symbol available"),
        };
        self.bug_records.df_bugs_output(fn_name);
        self.bug_records.uaf_bugs_output(fn_name);
        self.bug_records.dp_bug_output(fn_name);
    }

    pub fn uaf_check(&mut self, aliaset_idx: usize, span: Span, local: usize, is_func_call: bool) {
        let mut record = FxHashSet::default();
        if self.values[aliaset_idx].may_drop 
            && (!self.values[aliaset_idx].is_ptr() 
                || self.values[aliaset_idx].local != local
                || is_func_call)
            && self.exist_dead(aliaset_idx, &mut record, false)
            && !self.bug_records.uaf_bugs.contains(&span) {            
            self.bug_records.uaf_bugs.insert(span.clone());
        }
    }

    pub fn exist_dead(&self, node: usize, record: &mut FxHashSet<usize>, dangling: bool) -> bool {
        //if is a dangling pointer check, only check the pointer type varible.
        if self.values[node].is_alive() == false && (dangling && self.values[node].is_ptr() || !dangling) {
            return true; 
        }
        record.insert(node);
        if self.values[node].alias[0] != node {
            for i in self.values[node].alias.clone().into_iter() {
                if i != node && record.contains(&i) == false && self.exist_dead(i, record, dangling) {
                    return true;
                }
            }
        }
        for i in self.values[node].fields.clone().into_iter() {
            if record.contains(&i.1) == false && self.exist_dead(i.1, record, dangling) {
                return true;
            }
        }
        return false;
    }

    pub fn is_dangling(&self, local: usize) -> bool {
        let mut record = FxHashSet::default();
        return self.exist_dead(local, &mut record, local != 0);
    }

    pub fn df_check(&mut self, drop: usize, span: Span) -> bool {
        let root = self.values[drop].local;
        if self.values[drop].is_alive() == false 
        && self.bug_records.df_bugs.contains_key(&root) == false {
            self.bug_records.df_bugs.insert(root, span.clone());
        }
        return self.values[drop].is_alive() == false;
    }

    pub fn dp_check(&mut self, current_block: &BlockNode<'tcx>) {
        match current_block.is_cleanup {
            true => {
                for i in 0..self.arg_size {
                    if self.values[i+1].is_ptr() && self.is_dangling(i+1) {
                        self.bug_records.dp_bugs_unwind.insert(self.span);
                    }
                }
            },
            false => { 
                if self.values[0].may_drop && self.is_dangling(0) {
                    self.bug_records.dp_bugs.insert(self.span);
                } else{
                    for i in 0..self.arg_size {
                        if self.values[i+1].is_ptr() && self.is_dangling(i+1) {
                            self.bug_records.dp_bugs.insert(self.span);
                        }
                    }
                }
            },
        }
    }

    pub fn dead_node(&mut self, drop: usize, birth: usize, info: &SourceInfo, alias: bool) {
        //Rc drop
        if self.values[drop].is_corner_case() {
            return;
        } 
        //check if there is a double free bug.
        if self.df_check(drop, info.span) {
            return;
        }
        //drop their alias
        if self.values[drop].alias[0] != drop {
            for i in self.values[drop].alias.clone().into_iter() {
                if self.values[i].is_ref(){
                    continue;
                }
                self.dead_node(i, birth, info, true);
            }
        }
        //drop the fields of the root node.
        //alias flag is used to avoid the fields of the alias are dropped repeatly.
        if alias == false {
            for i in self.values[drop].fields.clone().into_iter() {
                if self.values[drop].is_tuple() == true && self.values[i.1].need_drop == false {
                    continue;
                } 
                self.dead_node( i.1, birth, info, false);
            }
        }
        //SCC.
        if self.values[drop].birth < birth as isize && self.values[drop].may_drop {
            self.values[drop].dead();   
        }
    }

    //merge the result of current path to the final result.
    pub fn merge_results(&mut self, results_nodes: Vec<ValueNode>, is_cleanup: bool) {
        for node in results_nodes.iter() {
            if node.local <= self.arg_size {
                if node.alias[0] != node.index || node.alias.len() > 1 {
                    for alias in node.alias.clone(){
                        if results_nodes[alias].local <= self.arg_size
                        && !self.return_set.contains(&(node.index, alias))
                        && alias != node.index
                        && node.local != results_nodes[alias].local {
                            self.return_set.insert((node.index, alias));
                            let left_node = node;
                            let right_node = &results_nodes[alias];
                            let mut new_alias = RetAlias::new(0, 
                                left_node.local, left_node.may_drop, left_node.need_drop,
                                right_node.local, right_node.may_drop, right_node.need_drop
			                );
                            new_alias.left = self.get_field_seq(left_node); 
                            new_alias.right = self.get_field_seq(right_node); 
                            self.ret_alias.alias_vec.push(new_alias);
                        }
                    }
                }
                if node.is_ptr() && is_cleanup == false && node.is_alive() == false && node.index <= self.arg_size {
                    self.ret_alias.dead.insert(node.index);
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


