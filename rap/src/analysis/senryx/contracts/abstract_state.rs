use std::collections::HashSet;

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
pub struct AbstractStateItem<T: std::cmp::PartialEq + std::cmp::PartialOrd> {
    pub value: (T,T),
    pub state: HashSet<StateType>,
}

pub struct AbstractState {

}

impl AbstractState {
    pub fn new() -> Self {
        Self {

        }
    }
}