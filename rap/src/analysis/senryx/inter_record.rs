use lazy_static::lazy_static;
use rustc_hir::def_id::DefId;
use std::{collections::HashMap, sync::Mutex};

use super::contracts::abstract_state::AbstractStateItem;

lazy_static! {
    pub static ref GLOBAL_INTER_RECORDER: Mutex<HashMap<DefId, InterAnalysisRecord>> =
        Mutex::new(HashMap::new());
}
// static mut GLOBAL_INTER_RECORDER: HashMap<DefId,InterAnalysisRecord> = HashMap::new();

pub struct InterAnalysisRecord {
    pub pre_analysis_state: HashMap<usize, AbstractStateItem>,
    pub post_analysis_state: HashMap<usize, AbstractStateItem>,
}

impl InterAnalysisRecord {
    pub fn new(
        pre_analysis_state: HashMap<usize, AbstractStateItem>,
        post_analysis_state: HashMap<usize, AbstractStateItem>,
    ) -> Self {
        Self {
            pre_analysis_state,
            post_analysis_state,
        }
    }

    pub fn is_pre_state_same(&self, other_pre_state: &HashMap<usize, AbstractStateItem>) -> bool {
        self.pre_analysis_state == *other_pre_state
    }
}
