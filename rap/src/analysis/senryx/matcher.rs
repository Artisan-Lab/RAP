use rustc_middle::mir::Operand;

use super::contracts::{abstract_state::AbstractState, checker::{Checker, SliceFromRawPartsChecker}, contract::check_contract};

pub fn match_unsafe_api_and_check_contracts<T>(func_name: &str, args:&Vec<Operand>, abstate:&AbstractState, _ty: T) {
    let base_func_name = func_name.split::<&str>("<").next().unwrap_or(func_name);
    println!("base name ---- {:?}",base_func_name);
    let checker: Option<Box<dyn Checker>> = match base_func_name {
        "std::slice::from_raw_parts::" => {
            Some(Box::new(SliceFromRawPartsChecker::<T>::new()))
        }
        _ => None,
    };

    if let Some(c) = checker {
        process_checker(&*c, args, abstate);
    }
}

fn process_checker(checker: &dyn Checker, args: &Vec<Operand>, abstate: &AbstractState) {
    for (idx, contracts_vec) in checker.variable_contracts().iter() {
        for contract in contracts_vec {
            let arg_place = get_arg_place(&args[*idx]);
            if arg_place == 0 {
                return 
            }
            if let Some(abstate_item) = abstate.state_map.get(&arg_place){
                if !check_contract(*contract, abstate_item){
                    println!("Checking contract failed! ---- {:?}",contract);
                }
            }
        }
    }
}

pub fn get_arg_place(arg: &Operand) -> usize {
    match arg {
        Operand::Move(place) => { place.local.as_usize() }
        Operand::Copy(place) => { place.local.as_usize() }
        _ => { 0 }
    }
}