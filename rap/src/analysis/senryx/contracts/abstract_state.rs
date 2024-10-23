use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use super::state_lattice::Lattice;

#[derive(Debug, PartialEq, PartialOrd, Copy, Clone)]
pub enum Value {
    Usize(usize),
    Isize(isize),
    U32(u32),
    Custom(),
    None,
    // ...
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum StateType {
    AllocatedState(AllocatedState),
    AlignState(AlignState),
    // ...
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Op {
    EQ,
    NE,
    LT,
    GT,
    LE,
    GE,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum AllocatedState {
    Top,
    Borrowed,
    Moved,
    Alloc,
    SpecificAlloc,
    Bottom,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum AlignState {
    Aligned,
    Small2BigCast,
    Big2SmallCast,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum InitState {
    FullyInitialized,
    PartlyInitialized,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum VType {
    Pointer(usize, usize), // (align, size)
                           // todo
}

#[derive(Debug, PartialEq, Clone)]
pub struct AbstractStateItem {
    pub value: (Value, Value),
    pub vtype: VType,
    pub state: HashSet<StateType>,
}

impl AbstractStateItem {
    pub fn new(value: (Value, Value), vtype: VType, state: HashSet<StateType>) -> Self {
        Self {
            value,
            vtype,
            state,
        }
    }

    pub fn meet_state_item(&mut self, other_state: &AbstractStateItem) {
        let mut new_state = HashSet::new();

        // visit 'self.state' and 'other_state.state'，matching states and calling meet method
        for state_self in &self.state {
            // if find the same state type in 'other_state', then meet it;
            if let Some(matching_state) = other_state.state.iter().find(|state_other| {
                std::mem::discriminant(*state_other) == std::mem::discriminant(state_self)
            }) {
                let merged_state = match (state_self, matching_state) {
                    (StateType::AllocatedState(s1), StateType::AllocatedState(s2)) => {
                        StateType::AllocatedState(s1.meet(*s2))
                    }
                    (StateType::AlignState(s1), StateType::AlignState(s2)) => {
                        StateType::AlignState(s1.meet(*s2))
                    }
                    _ => continue,
                };
                new_state.insert(merged_state);
            } else {
                // if 'other_state' does not have the same state，then reserve the current state
                new_state.insert(*state_self);
            }
        }

        // 更新 self 的状态
        self.state = new_state;
    }
}

pub struct AbstractState {
    pub state_map: HashMap<usize, AbstractStateItem>,
}

impl AbstractState {
    pub fn new() -> Self {
        Self {
            state_map: HashMap::new(),
        }
    }

    pub fn insert_abstate(&mut self, place: usize, place_state_item: AbstractStateItem) {
        self.state_map.insert(place, place_state_item);
    }
}
