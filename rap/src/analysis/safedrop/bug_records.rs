use rustc_span::Span;
use rustc_span::symbol::Symbol;
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use crate::{rap_warn};

//structure to record the existed bugs.
pub struct BugRecords {
    pub df_bugs: FxHashMap<usize, Span>,
    pub df_bugs_unwind: FxHashMap<usize, Span>,
    pub uaf_bugs: FxHashSet<Span>,
    pub dp_bugs: FxHashSet<Span>,
    pub dp_bugs_unwind: FxHashSet<Span>,
}

impl BugRecords{
    pub fn new() -> BugRecords {
        BugRecords { df_bugs: FxHashMap::default(), df_bugs_unwind: FxHashMap::default(), uaf_bugs: FxHashSet::default(), dp_bugs: FxHashSet::default(), dp_bugs_unwind: FxHashSet::default()}
    }

    pub fn is_bug_free(&self) -> bool {
        self.df_bugs.is_empty() && self.uaf_bugs.is_empty() && self.dp_bugs.is_empty() && self.dp_bugs_unwind.is_empty()
    }

    pub fn df_bugs_output(&self, fn_name:Symbol) {
        if !self.df_bugs.is_empty() {
            rap_warn!("Double free detected in function {:}", fn_name);
            for i in self.df_bugs.iter() {
                rap_warn!("Location: {:?}", i.1);
            }
        }
    }

    pub fn uaf_bugs_output(&self, fn_name:Symbol) {
        if !self.uaf_bugs.is_empty() {
            rap_warn!("Use after free detected in function {:?}", fn_name);
            for i in self.uaf_bugs.iter() {
                rap_warn!("Location: {:?}", i);
            }
        }

    }

    pub fn dp_bug_output(&self, fn_name:Symbol) {
        for i in self.dp_bugs.iter() {
            rap_warn!("Dangling pointer detected in function {:?}!!! 
                      Location: {:?}", fn_name, i);
        }
        for i in self.dp_bugs_unwind.iter() {
            rap_warn!("Dangling pointer detected in function {:?}!!!
                      Location: {:?} during unwinding.", fn_name, i);
        }
    }
}
