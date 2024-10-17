pub mod mop;

use rustc_data_structures::fx::FxHashMap;
use rustc_hir::def_id::DefId;
use std::{collections::HashSet, fmt};

//struct to cache the results for analyzed functions.
pub type FnMap = FxHashMap<DefId, FnRetAlias>;

/*
 * To store the alias relationships among arguments and return values.
 * Each function may have multiple return instructions, leading to different RetAlias.
 */
#[derive(Debug, Clone)]
pub struct FnRetAlias {
    arg_size: usize,
    alias_set: HashSet<RetAlias>,
}

impl FnRetAlias {
    pub fn new(arg_size: usize) -> FnRetAlias {
        Self {
            arg_size,
            alias_set: HashSet::new(),
        }
    }

    pub fn arg_size(&self) -> usize {
        self.arg_size
    }

    pub fn aliases(&self) -> &HashSet<RetAlias> {
        &self.alias_set
    }

    pub fn add_alias(&mut self, alias: RetAlias) {
        self.alias_set.insert(alias);
    }

    pub fn len(&self) -> usize {
        self.alias_set.len()
    }
}

impl fmt::Display for FnRetAlias {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{{}}}",
            self.aliases()
                .iter()
                .map(|alias| format!("{}", alias))
                .collect::<Vec<String>>()
                .join(",")
        )
    }
}

/*
 * To store the alias relationships among arguments and return values.
 */
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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
            left_index,
            left_field_seq: Vec::<usize>::new(),
            left_may_drop,
            left_need_drop,
            right_index,
            right_field_seq: Vec::<usize>::new(),
            right_may_drop,
            right_need_drop,
        }
    }

    fn get_index(index: usize, fields: &[usize], field_sensitive: bool) -> String {
        let mut result = String::new();
        result.push_str(&index.to_string());
        if !field_sensitive {
            return result;
        }
        for num in fields.iter() {
            result.push('.');
            result.push_str(&num.to_string());
        }
        result
    }

    fn lhs_str(&self, field_sensitive: bool) -> String {
        Self::get_index(self.left_index, &self.left_field_seq, field_sensitive)
    }

    fn rhs_str(&self, field_sensitive: bool) -> String {
        Self::get_index(self.right_index, &self.right_field_seq, field_sensitive)
    }

    pub fn valuable(&self) -> bool {
        return self.left_may_drop && self.right_may_drop;
    }
}

impl fmt::Display for RetAlias {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({},{})", self.lhs_str(true), self.rhs_str(true))
    }
}
