use core::mem;
use super::abstract_state::*;
use super::contract::*;
use std::collections::HashMap;
use std::marker::PhantomData;

pub struct SliceFromRawPartsChecker<T>{
    pub variable_contracts: HashMap<usize,Vec<Contract>>,
    _marker: PhantomData<T>,
}

impl<T> SliceFromRawPartsChecker<T> {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(0, vec![
            Contract::ValueCheck { op: Op::GE, value: Value::Usize(0) },
            Contract::StateCheck { op: Op::EQ, state: StateType::AllocatedState(AllocatedState::Alloc) },
        ]);
        map.insert(1, vec![
            Contract::ValueCheck { op: Op::LE, value: Value::Isize((isize::MAX)/(mem::size_of::<T>() as isize)) },
        ]);
        Self {
            variable_contracts: map,
            _marker: PhantomData,
        }
    }
}