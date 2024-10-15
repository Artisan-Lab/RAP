use super::abstract_state::*;
use crate::analysis::senryx::contracts::state_lattice::Lattice;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Contract {
    ValueCheck { op: Op, value: Value },
    StateCheck { op: Op, state: StateType },
}

pub fn check_contract(contract: Contract, abstate_item: &AbstractStateItem) -> bool {
    match contract {
        Contract::ValueCheck { op, value } => {
            return handle_value_op(&abstate_item.value, op, value);
        }
        Contract::StateCheck { op, state } => {
            for ab_state in &abstate_item.state {
                if handle_state_op(*ab_state, op, state) {
                    return true;
                }
            }
            return false;
        }
    }
}

pub fn handle_value_op<T: std::cmp::PartialEq + std::cmp::PartialOrd>(
    left: &(T, T),
    op: Op,
    right: T,
) -> bool {
    match op {
        Op::EQ => {
            return left.0 == right;
        }
        Op::NE => {
            return left.0 != right;
        }
        Op::LT => {
            return left.1 < right;
        }
        Op::GT => {
            return left.0 > right;
        }
        Op::LE => {
            return left.1 <= right;
        }
        Op::GE => {
            return left.0 >= right;
        }
    }
}

pub fn handle_state_op(left: StateType, op: Op, right: StateType) -> bool {
    match op {
        Op::LT => left.less_than(right),
        Op::LE => left.less_than(right) || left.equal(right),
        Op::GT => right.less_than(left),
        Op::GE => right.less_than(left) || right.equal(left),
        Op::EQ => left.equal(right),
        Op::NE => !left.equal(right),
    }
}
