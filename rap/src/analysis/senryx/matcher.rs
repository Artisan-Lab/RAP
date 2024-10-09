use rustc_middle::mir::Operand;

use super::contracts::{abstract_state::AbstractState, checker::SliceFromRawPartsChecker, contract::check_contract};

pub fn match_unsafe_api_and_check_contracts(func_name: &str, args:&Vec<Operand>, abstate:&AbstractState) {
    match func_name {
        "slice" => {
            let checker: SliceFromRawPartsChecker<usize>  = SliceFromRawPartsChecker::new();
            for (idx, contracts_vec) in checker.variable_contracts {
                for contract in contracts_vec {
                    let arg_place = get_arg_place(&args[idx]);
                    if arg_place == 0 {
                        return 
                    }
                    if let Some(abstate_item) = abstate.state_map.get(&arg_place){
                        if !check_contract(contract, abstate_item){
                            println!("Checking contract failed! ---- {:?}",contract);
                        }
                    }
                }
            }
        }
        _ => { return }
    }
}

pub fn get_arg_place(arg: &Operand) -> usize {
    match arg {
        Operand::Move(place) => { place.local.as_usize() }
        Operand::Copy(place) => { place.local.as_usize() }
        _ => { 0 }
    }
}