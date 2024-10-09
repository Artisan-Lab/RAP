use std::{collections::{HashMap, HashSet}, hash::Hash};

#[derive(Debug, PartialEq, PartialOrd, Copy, Clone)]
pub enum Value {
    Usize(usize),
    Isize(isize),
    U32(u32),
    Custom(),
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
    Unaligned,
}

#[derive(Debug, PartialEq)]
pub struct AbstractStateItem {
    pub value: (Value,Value),
    pub state: HashSet<StateType>,
}

pub struct AbstractState {
    pub state_map: HashMap<usize,AbstractStateItem>,
}

impl AbstractState {
    pub fn new() -> Self {
        Self {
            state_map: HashMap::new(),
        }
    }
}