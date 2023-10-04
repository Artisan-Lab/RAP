pub mod connect;
pub mod type_visitor;
pub mod ownership;

use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::def_id::DefId;

use crate::analysis::RcxMut;
use crate::analysis::type_analysis::ownership::RawTypeOwner;
use crate::components::context::RapGlobalCtxt;

use std::collections::{HashMap, HashSet};
use std::env;

use stopwatch::Stopwatch;

type TyMap<'tcx> = HashMap<Ty<'tcx>, String>;
type OwnerUnit = (RawTypeOwner, Vec<bool>);
pub type AdtOwner = HashMap<DefId, Vec<OwnerUnit>>;
type Parameters = HashSet<usize>;
pub type Unique = HashSet<DefId>;
pub type OwnershipLayout = Vec<RawTypeOwner>;
pub type RustBV = Vec<bool>;

// Type Analysis is the first step and it will perform a simple-inter-procedural analysis
// for current crate and collect types after monomorphism as well as extracting 'adt-def'.
// The struct TypeAnalysis implements mir::Visitor to simulate as the type collector.
// Note: the type in this phase is Ty::ty rather of Hir::ty.
pub struct TypeAnalysis<'tcx, 'a> {
    rcx: &'a mut RapGlobalCtxt<'tcx>,
    fn_set: Unique,
    ty_map: TyMap<'tcx>,
    adt_recorder: Unique,
}

impl<'tcx, 'a> TypeAnalysis<'tcx, 'a> {
    pub fn new(rcx: &'a mut RapGlobalCtxt<'tcx>) -> Self {
        Self {
            rcx,
            fn_set: HashSet::new(),
            ty_map: HashMap::new(),
            adt_recorder: HashSet::new(),
        }
    }

    pub fn ty_map(&self) -> &TyMap<'tcx> {
        &self.ty_map
    }

    pub fn ty_map_mut(&mut self) -> &mut TyMap<'tcx> {
        &mut self.ty_map
    }

    pub fn fn_set(&self) -> &Unique {
        &self.fn_set
    }

    pub fn fn_set_mut(&mut self) -> &mut Unique {
        &mut self.fn_set
    }

    pub fn adt_recorder(&self) -> &Unique {
        &self.adt_recorder
    }

    pub fn adt_recorder_mut(&mut self) -> &mut Unique {
        &mut self.adt_recorder
    }

    pub fn adt_owner(&self) -> &AdtOwner {
        self.rcx().adt_owner()
    }

    pub fn adt_owner_mut(&mut self) -> &mut AdtOwner {
        self.rcx_mut().adt_owner_mut()
    }

    // The main phase and the starter function of Type Collector.
    // RAP will construct an instance of struct TypeCollector and call self.start to make analysis starting.
    pub fn start(&mut self) {

        let mut sw = Stopwatch::start_new();

        // Get the analysis result from rap phase llvm
        // self.connect();
        // Get related adt types through visiting mir local
        self.visitor();

        //rap_info!("AdtDef Sum:{:?}", self.adt_owner().len());
        //rap_info!("Tymap Sum:{:?}", self.ty_map().len());
        //rap_info!("@@@@@@@@@@@@@Type Analysis:{:?}", sw.elapsed_ms());
        sw.stop();
    }
}

impl<'tcx, 'o, 'a> RcxMut<'tcx, 'o, 'a> for TypeAnalysis<'tcx, 'a> {
    #[inline(always)]
    fn rcx(&'o self) -> &'o RapGlobalCtxt<'tcx> {
        self.rcx
    }

    #[inline(always)]
    fn rcx_mut(&'o mut self) -> &'o mut RapGlobalCtxt<'tcx> {
        &mut self.rcx
    }

    #[inline(always)]
    fn tcx(&'o self) -> TyCtxt<'tcx> {
        self.rcx().tcx()
    }
}

#[derive(Clone)]
struct RawGeneric<'tcx> {
    tcx: TyCtxt<'tcx>,
    record: Vec<bool>,
}

impl<'tcx> RawGeneric<'tcx> {

    pub fn new(
        tcx: TyCtxt<'tcx>,
        len: usize
    ) -> Self
    {
        Self {
            tcx,
            record: vec![false ; len],
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn record(&self) -> &Vec<bool> {
        &self.record
    }

    pub fn record_mut(&mut self) -> &mut Vec<bool> {
        &mut self.record
    }
}

#[derive(Clone)]
struct RawGenericFieldSubst<'tcx> {
    tcx: TyCtxt<'tcx>,
    parameters: Parameters,
}

impl<'tcx> RawGenericFieldSubst<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            parameters: HashSet::new(),
        }
    }
    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn parameters(&self) -> &Parameters {
        &self.parameters
    }

    pub fn parameters_mut(&mut self) -> &mut Parameters {
        &mut self.parameters
    }

    pub fn contains_param(&self) -> bool {
        !self.parameters.is_empty()
    }

}


#[derive(Clone)]
struct RawGenericPropagation<'tcx, 'a> {
    tcx: TyCtxt<'tcx>,
    record: Vec<bool>,
    unique: Unique,
    source_enum: bool,
    ref_adt_owner: &'a AdtOwner,
}

impl<'tcx, 'a> RawGenericPropagation<'tcx, 'a> {
    pub fn new(
        tcx: TyCtxt<'tcx>,
        record: Vec<bool>,
        source_enum: bool,
        ref_adt_owner: &'a AdtOwner
    ) -> Self
    {
        Self {
            tcx,
            record,
            unique: HashSet::new(),
            source_enum,
            ref_adt_owner,
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn record(&self) -> &Vec<bool> {
        &self.record
    }

    pub fn record_mut(&mut self) -> &mut Vec<bool> {
        &mut self.record
    }

    pub fn unique(&self) -> &Unique {
        &self.unique
    }

    pub fn unique_mut(&mut self) -> &mut Unique {
        &mut self.unique
    }

    pub fn source_enum(&mut self) -> bool {
        self.source_enum
    }

    pub fn owner(&self) -> &'a AdtOwner {
        self.ref_adt_owner
    }

}

#[derive(Clone)]
struct OwnerPropagation<'tcx, 'a> {
    tcx: TyCtxt<'tcx>,
    ownership: RawTypeOwner,
    unique: Unique,
    ref_adt_owner: &'a AdtOwner,
}

impl<'tcx, 'a> OwnerPropagation<'tcx, 'a> {
    pub fn new(
        tcx: TyCtxt<'tcx>,
        ownership: RawTypeOwner,
        ref_adt_owner: &'a AdtOwner
    ) -> Self
    {
        Self {
            tcx,
            ownership,
            unique: HashSet::new(),
            ref_adt_owner,
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn ownership(&self) -> RawTypeOwner {
        self.ownership
    }

    pub fn unique(&self) -> &Unique {
        &self.unique
    }

    pub fn unique_mut(&mut self) -> &mut Unique {
        &mut self.unique
    }

    pub fn owner(&self) -> &'a AdtOwner {
        self.ref_adt_owner
    }

}

#[derive(Clone)]
pub struct DefaultOwnership<'tcx, 'a> {
    tcx: TyCtxt<'tcx>,
    unique: Unique,
    ref_adt_owner: &'a AdtOwner,
    res: RawTypeOwner,
    param: bool,
    ptr: bool,
}

impl<'tcx, 'a> DefaultOwnership<'tcx, 'a> {
    pub fn new(
        tcx: TyCtxt<'tcx>,
        ref_adt_owner: &'a AdtOwner
    ) -> Self
    {
        Self {
            tcx,
            unique: HashSet::new(),
            ref_adt_owner,
            res: RawTypeOwner::Unowned,
            param: false,
            ptr: false,
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn unique(&self) -> &Unique {
        &self.unique
    }

    pub fn unique_mut(&mut self) -> &mut Unique {
        &mut self.unique
    }

    pub fn get_res(&self) -> RawTypeOwner {
        self.res
    }

    pub fn set_res(&mut self, res: RawTypeOwner) {
        self.res = res;
    }

    pub fn is_owning_true(&self) -> bool {
        self.res == RawTypeOwner::Owned
    }

    pub fn get_param(&self) -> bool {
        self.param
    }

    pub fn set_param(&mut self, p: bool) {
        self.param = p;
    }

    pub fn is_param_true(&self) -> bool {
        self.param == true
    }

    pub fn get_ptr(&self) -> bool {
        self.ptr
    }

    pub fn set_ptr(&mut self, p: bool) {
        self.ptr = p;
    }

    pub fn is_ptr_true(&self) -> bool {
        self.ptr == true
    }

    pub fn owner(&self) -> &'a AdtOwner {
        self.ref_adt_owner
    }

}

#[derive(Clone)]
pub struct FindPtr<'tcx> {
    tcx: TyCtxt<'tcx>,
    unique: Unique,
    ptr: bool,
}

impl<'tcx> FindPtr<'tcx> {
    pub fn new(
        tcx: TyCtxt<'tcx>,
    ) -> Self
    {
        Self {
            tcx,
            unique: Unique::default(),
            ptr: false,
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn unique(&self) -> &Unique {
        &self.unique
    }

    pub fn unique_mut(&mut self) -> &mut Unique {
        &mut self.unique
    }

    pub fn has_ptr(&self) -> bool {
        self.ptr
    }

    pub fn set_ptr(&mut self, ptr: bool) {
        self.ptr = ptr;
    }
}

#[derive(Debug, Copy, Clone, Hash)]
pub enum AdtOwnerDisplay {
    Verbose,
    Disabled,
}

pub fn is_display_verbose() -> bool {
    match env::var_os("ADT_DISPLAY") {
        Some(_)  => true,
        _ => false,
    }
}

pub fn mir_body(tcx: TyCtxt<'_>, def_id: DefId) -> &rustc_middle::mir::Body<'_> {
    let def = ty::InstanceDef::Item(def_id);
    tcx.instance_mir(def)
}