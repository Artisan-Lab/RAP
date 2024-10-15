pub mod mop;

use rustc_data_structures::fx::FxHashMap;
use rustc_hir::def_id::DefId;
use std::fmt;

//struct to cache the results for analyzed functions.
pub type FnMap = FxHashMap<DefId, FnRetAlias>;

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
    pub fn new(arg_size: usize) -> FnRetAlias {
        Self {
            arg_size: arg_size,
            alias_vec: Vec::<RetAlias>::new(),
        }
    }
}

impl fmt::Display for FnRetAlias {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{}]",
            self.alias_vec
                .iter()
                .map(|alias| format!("{}", alias))
                .collect::<Vec<String>>()
                .join("")
        )
    }
}

/*
 * To store the alias relationships among arguments and return values.
 */
#[derive(Debug, Clone)]
pub struct RetAlias {
    pub left_index: usize,
    pub left_field_seq: Vec<usize>,
    pub left_may_drop: bool,
    pub left_need_drop: bool,
    pub right_index: usize,
    pub right_field_seq: Vec<usize>,
    pub right_may_drop: bool,
    pub right_need_drop: bool,
}

impl RetAlias {
    pub fn new(
        left_index: usize,
        left_may_drop: bool,
        left_need_drop: bool,
        right_index: usize,
        right_may_drop: bool,
        right_need_drop: bool,
    ) -> RetAlias {
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

    pub fn valuable(&self) -> bool {
        return self.left_may_drop && self.right_may_drop;
    }
}

impl fmt::Display for RetAlias {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({},{})", self.left_index, self.right_index)
    }
}
