use super::contracts::abstract_state::AbstractState;
use super::matcher::match_unsafe_api_and_check_contracts;
use rustc_middle::ty::TyCtxt;
use rustc_middle::{
    ty,
    mir::{self, ProjectionElem, Terminator, TerminatorKind, Operand, Statement, StatementKind, Place, Rvalue, AggregateKind, BasicBlockData, BasicBlock},
};
use rustc_hir::def_id::DefId;

pub struct BodyVisitor<'tcx>  {
    pub tcx: TyCtxt<'tcx>,
    pub abstract_states: AbstractState,
}

impl<'tcx> BodyVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self{
        Self{
            tcx,
            abstract_states: AbstractState::new(),
        }
    }

    pub fn path_forward_check(&mut self, def_id: DefId) {
        let paths = self.get_all_paths(def_id);
        let body = self.tcx.optimized_mir(def_id);
        for (index, path_info) in paths.iter().enumerate() {
            for block_index in path_info.iter() {
                if block_index >= &body.basic_blocks.len(){
                    continue;
                }
                self.path_analyze_block(&body.basic_blocks[BasicBlock::from_usize(*block_index)].clone(), index, *block_index);
                // TODO: analyze scc blocks
                // let tem_scc_sub_blocks = self.scc_sub_blocks[*block_index].clone();
                // if tem_scc_sub_blocks.len() > 0{
                //     for sub_block in &tem_scc_sub_blocks {
                //         self.path_analyze_block(tcx, &self.body.as_ref().basic_blocks[BasicBlock::from_usize(*sub_block)].clone(), index, *block_index,direction);
                //     }
                // }
            }
        }
    }

    pub fn path_analyze_block(&mut self, block:&BasicBlockData<'tcx>, path_index:usize, bb_index: usize,) {
        for statement in block.statements.iter().rev() {
            self.path_analyze_statement(statement,path_index);
        }
        self.path_analyze_terminator(&block.terminator(), path_index, bb_index);
    }

    pub fn path_analyze_terminator(&mut self, terminator:&Terminator<'tcx>, _path_index:usize, _bb_index: usize) {
        match &terminator.kind {
            TerminatorKind::Call{func, args, destination: _, target: _, ..} => {
                let func_name = format!("{:?}",func);
                match_unsafe_api_and_check_contracts(func_name.as_str(), args, &self.abstract_states);

                //handle inter analysis
                if let Operand::Constant(func_constant) = func{
                    if let ty::FnDef(ref _callee_def_id, raw_list) = func_constant.const_.ty().kind() {
                        println!("{:?}",raw_list);
                        //TODO:path_inter_analyze
                    }
                }
            }
            _ => {}
        }
    }

    pub fn path_analyze_statement(&mut self, statement:&Statement<'tcx>, _path_index:usize) {
        match statement.kind {
            StatementKind::Assign(
                box(ref lplace, ref rvalue)
            ) => {
                self.path_analyze_assign(lplace, rvalue, _path_index);
            }
            StatementKind::Intrinsic(
                box ref intrinsic
            ) => {
                match intrinsic{
                    mir::NonDivergingIntrinsic::CopyNonOverlapping(cno) => {
                        if cno.src.place().is_some() && cno.dst.place().is_some() {
                            let src_place_local = cno.src.place().unwrap().local.as_usize();
                            let dst_place_local = cno.dst.place().unwrap().local.as_usize();
                            let _src_pjc_local = self.handle_projection(true, src_place_local, cno.src.place().unwrap().clone());
                            let _dst_pjc_local = self.handle_projection(true, dst_place_local, cno.dst.place().unwrap().clone());
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub fn path_analyze_assign(&mut self, lplace: &Place<'tcx>, rvalue: &Rvalue<'tcx>, _path_index: usize) {
        let llocal = lplace.local.as_usize();
        let _lpjc_local = self.handle_projection(false, llocal, lplace.clone());
        match rvalue {
            Rvalue::Use(op) => {
                match op {
                    Operand::Move(rplace) | Operand::Copy(rplace) => {
                        let rlocal = rplace.local.as_usize();
                        let _rpjc_local = self.handle_projection(true, rlocal, rplace.clone());
                    }
                    _ => {} 
                }
            }
            Rvalue::Repeat(op,_const) => {
                match op {
                    Operand::Move(rplace) | Operand::Copy(rplace) => {
                        let rlocal = rplace.local.as_usize();
                        let _rpjc_local = self.handle_projection(true, rlocal, rplace.clone());
                    }
                    _ => {}
                }
            }
            Rvalue::Ref(_,_,rplace) => {
                let rlocal = rplace.local.as_usize();
                let _rpjc_local = self.handle_projection(true, rlocal, rplace.clone());
            }
            Rvalue::AddressOf(_,rplace) => {
                let rlocal = rplace.local.as_usize();
                let _rpjc_local = self.handle_projection(true, rlocal, rplace.clone());
            }
            Rvalue::Cast(_cast_kind,op,_ty) => {
                match op {
                    Operand::Move(rplace) | Operand::Copy(rplace) => {
                        let rlocal = rplace.local.as_usize();
                        let _rpjc_local = self.handle_projection(true, rlocal, rplace.clone());
                    }
                    _ => {}
                }
            }
            Rvalue::BinaryOp(_bin_op,box(ref _op1, ref _op2)) => {
                
            }
            Rvalue::ShallowInitBox(op,_ty) => {
                match op {
                    Operand::Move(rplace) | Operand::Copy(rplace) => {
                        let rlocal = rplace.local.as_usize();
                        let _rpjc_local = self.handle_projection(true, rlocal, rplace.clone());
                    }
                    _ => {}
                }
            }
            Rvalue::Aggregate(box ref agg_kind, _op_vec) => {
                match agg_kind {
                    AggregateKind::Array(_ty) => {
                    }
                    _ => {}
                }
            }
            // Rvalue::Discriminant(_place) => {
            //     println!("{}:{:?}",llocal,rvalue);
            // }
            _ => {}
        }
    }

    pub fn handle_projection(&mut self, _is_right: bool, local: usize, place: Place<'tcx>) -> usize{
        let _init_local = local;
        let current_local = local;
        for projection in place.projection{
            match projection{
                ProjectionElem::Deref => {
                    
                }
                ProjectionElem::Field(_field, _ty) =>{
                    
                }
                _ => {}
            }
        }
        return current_local;
    }

    pub fn get_all_paths(&self, def_id: DefId) -> Vec<Vec<usize>> {
        let results = Vec::new();
        let _body = self.tcx.optimized_mir(def_id);
        // TODO: get all paths in a body
        results
    }

    pub fn get_all_callees(&self, def_id: DefId) -> Vec<String> {
        let mut results = Vec::new();
        let body = self.tcx.optimized_mir(def_id);
        let bb_len = body.basic_blocks.len();
        for i in 0..bb_len {
            let callees = Self::get_terminator_callee(body.basic_blocks[BasicBlock::from_usize(i)].clone().terminator());
            results.extend(callees);
        }
        results
    }

    pub fn get_terminator_callee(terminator:&Terminator<'tcx>) -> Vec<String> {
        let mut results = Vec::new();
        match &terminator.kind {
            TerminatorKind::Call{func, ..} => {
                let func_name = format!("{:?}",func);
                results.push(func_name);
            }
            _ => {}
        }
        results
    }
}