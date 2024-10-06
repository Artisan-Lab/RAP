use core::mem;
use super::abstract_state::*;
use super::contract::*;
use std::collections::HashMap;
use std::marker::PhantomData;

struct SliceFromRawPartsChecker<T>{
    variable_contracts: HashMap<usize,Vec<Contract<isize>>>,
    _marker: PhantomData<T>,
}

impl<T> SliceFromRawPartsChecker<T> {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(1, vec![
            Contract::ValueCheck { op: Op::GE, value: 0 },
            Contract::StateCheck { op: Op::EQ, state: StateType::AllocatedState(AllocatedState::Alloc) },
        ]);
        map.insert(2, vec![
            Contract::ValueCheck { op: Op::LE, value: (isize::MAX)/(mem::size_of::<T>() as isize) },
        ]);
        Self {
            variable_contracts: map,
            _marker: PhantomData,
        }
    }
}