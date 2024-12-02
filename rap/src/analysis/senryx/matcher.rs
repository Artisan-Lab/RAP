use rustc_middle::mir::Operand;
use rustc_span::source_map::Spanned;
use rustc_span::Span;

use super::{
    contracts::{
        abstract_state::AbstractState,
        checker::{Checker, SliceFromRawPartsChecker},
        contract::check_contract,
    },
    visitor::CheckResult,
};

pub fn match_unsafe_api_and_check_contracts<T>(
    func_name: &str,
    args: &Box<[Spanned<Operand>]>,
    abstate: &AbstractState,
    span: Span,
    _ty: T,
) -> Option<CheckResult> {
    let base_func_name = func_name.split::<&str>("<").next().unwrap_or(func_name);
    // println!("base name ---- {:?}",base_func_name);
    let checker: Option<Box<dyn Checker>> = match base_func_name {
        "std::slice::from_raw_parts::" | "std::slice::from_raw_parts_mut::" => {
            Some(Box::new(SliceFromRawPartsChecker::<T>::new()))
        }
        _ => None,
    };

    if let Some(c) = checker {
        return Some(process_checker(&*c, args, abstate, base_func_name, span));
    }
    None
}

fn process_checker(
    checker: &dyn Checker,
    args: &Box<[Spanned<Operand>]>,
    abstate: &AbstractState,
    func_name: &str,
    span: Span,
) -> CheckResult {
    let mut check_result = CheckResult::new(func_name, span);
    for (idx, contracts_vec) in checker.variable_contracts().iter() {
        for contract in contracts_vec {
            let arg_place = get_arg_place(&args[*idx].node);
            if arg_place == 0 {
                continue;
            }
            if let Some(abstate_item) = abstate.state_map.get(&arg_place) {
                if !check_contract(*contract, &abstate_item.clone().unwrap()) {
                    check_result.failed_contracts.push((*idx, *contract));
                } else {
                    check_result.passed_contracts.push((*idx, *contract));
                }
            }
        }
    }
    check_result
}

pub fn get_arg_place(arg: &Operand) -> usize {
    match arg {
        Operand::Move(place) => place.local.as_usize(),
        Operand::Copy(place) => place.local.as_usize(),
        _ => 0,
    }
}
