use super::graph::*;
use super::*;
use rustc_data_structures::fx::FxHashSet;
use rustc_hir::def_id::DefId;
use rustc_middle::mir::Operand::{Constant, Copy, Move};
use rustc_middle::mir::TerminatorKind;

impl<'tcx> MopGraph<'tcx> {
    pub fn split_check(
        &mut self,
        bb_index: usize,
        fn_map: &mut FnMap,
        recursion_set: &mut FxHashSet<DefId>,
    ) {
        /* duplicate the status before visiting a path; */
        let backup_values = self.values.clone(); // duplicate the status when visiting different paths;
        let backup_constant = self.constant.clone();
        let backup_alias_set = self.alias_set.clone();
        self.check(bb_index, fn_map, recursion_set);
        /* restore after visit */
        self.alias_set = backup_alias_set;
        self.values = backup_values;
        self.constant = backup_constant;
    }
    pub fn split_check_with_cond(
        &mut self,
        bb_index: usize,
        path_discr_id: usize,
        path_discr_val: usize,
        fn_map: &mut FnMap,
        recursion_set: &mut FxHashSet<DefId>,
    ) {
        /* duplicate the status before visiting a path; */
        let backup_values = self.values.clone(); // duplicate the status when visiting different paths;
        let backup_constant = self.constant.clone();
        let backup_alias_set = self.alias_set.clone();
        /* add control-sensitive indicator to the path status */
        self.constant.insert(path_discr_id, path_discr_val);
        self.check(bb_index, fn_map, recursion_set);
        /* restore after visit */
        self.alias_set = backup_alias_set;
        self.values = backup_values;
        self.constant = backup_constant;
    }

    // the core function of the safedrop.
    pub fn check(
        &mut self,
        bb_index: usize,
        fn_map: &mut FnMap,
        recursion_set: &mut FxHashSet<DefId>,
    ) {
        self.visit_times += 1;
        if self.visit_times > VISIT_LIMIT {
            return;
        }
        let cur_block = self.blocks[self.scc_indices[bb_index]].clone();
        self.alias_bb(self.scc_indices[bb_index]);
        self.alias_bbcall(self.scc_indices[bb_index], fn_map, recursion_set);

        /* Handle cases if the current block is a merged scc block with sub block */
        if !cur_block.scc_sub_blocks.is_empty() {
            for i in cur_block.scc_sub_blocks.clone() {
                self.alias_bb(i);
                self.alias_bbcall(i, fn_map, recursion_set);
            }
        }

        /* Reach a leaf node, check bugs */
        match cur_block.next.len() {
            0 => {
                let results_nodes = self.values.clone();
                self.merge_results(results_nodes);
                return;
            }
            1 => {
                /*
                 * Equivalent to self.check(cur_block.next[0]..);
                 * We cannot use [0] for FxHashSet.
                 */
                for next in cur_block.next {
                    self.check(next, fn_map, recursion_set);
                }
                return;
            }
            _ => { // multiple blocks
            }
        }

        /* Begin: handle the SwitchInt statement. */
        let mut single_target = false;
        let mut sw_val = 0;
        let mut sw_target = 0; // Single target
        let mut path_discr_id = 0; // To avoid analyzing paths that cannot be reached with one enum type.
        let mut sw_targets = None; // Multiple targets of SwitchInt
        if !cur_block.switch_stmts.is_empty() && cur_block.scc_sub_blocks.is_empty() {
            if let TerminatorKind::SwitchInt {
                ref discr,
                ref targets,
            } = cur_block.switch_stmts[0].clone().kind
            {
                match discr {
                    Copy(p) | Move(p) => {
                        let place = self.projection(false, *p);
                        if let Some(constant) = self.constant.get(&self.values[place].index) {
                            single_target = true;
                            sw_val = *constant;
                        }
                        if self.values[place].index != place {
                            path_discr_id = self.values[place].index;
                            sw_targets = Some(targets.clone());
                        }
                    }
                    Constant(c) => {
                        single_target = true;
                        let param_env = self.tcx.param_env(self.def_id);
                        if let Some(val) = c.const_.try_eval_target_usize(self.tcx, param_env) {
                            sw_val = val as usize;
                        }
                    }
                }
                if single_target {
                    /* Find the target based on the value;
                     * Since sw_val is a const, only one target is reachable.
                     * Filed 0 is the value; field 1 is the real target.
                     */
                    for iter in targets.iter() {
                        if iter.0 as usize == sw_val {
                            sw_target = iter.1.as_usize();
                            break;
                        }
                    }
                    /* No target found, choose the default target.
                     * The default targets is not included within the iterator.
                     * We can only obtain the default target based on the last item of all_targets().
                     */
                    if sw_target == 0 {
                        let all_target = targets.all_targets();
                        sw_target = all_target[all_target.len() - 1].as_usize();
                    }
                }
            }
        }
        /* End: finish handling SwitchInt */
        // fixed path since a constant switchInt value
        if single_target {
            self.check(sw_target, fn_map, recursion_set);
        } else {
            // Other cases in switchInt terminators
            if let Some(targets) = sw_targets {
                for iter in targets.iter() {
                    if self.visit_times > VISIT_LIMIT {
                        continue;
                    }
                    let next_index = iter.1.as_usize();
                    let path_discr_val = iter.0 as usize;
                    self.split_check_with_cond(
                        next_index,
                        path_discr_id,
                        path_discr_val,
                        fn_map,
                        recursion_set,
                    );
                }
                let all_targets = targets.all_targets();
                let next_index = all_targets[all_targets.len() - 1].as_usize();
                let path_discr_val = usize::MAX; // to indicate the default path;
                self.split_check_with_cond(
                    next_index,
                    path_discr_id,
                    path_discr_val,
                    fn_map,
                    recursion_set,
                );
            } else {
                for i in cur_block.next {
                    if self.visit_times > VISIT_LIMIT {
                        continue;
                    }
                    let next_index = i;
                    self.split_check(next_index, fn_map, recursion_set);
                }
            }
        }
    }
}
