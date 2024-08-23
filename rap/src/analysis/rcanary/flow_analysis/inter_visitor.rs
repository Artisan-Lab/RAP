use super::super::RcxMut;
use super::super::type_analysis::mir_body;
use super::FlowAnalysis;

impl<'tcx, 'a> FlowAnalysis<'tcx, 'a> {
    pub fn inter_run(&mut self) {
        let tcx = self.tcx();
        let mir_keys = tcx.mir_keys(());

        for each_mir in mir_keys {
            //let sw = Stopwatch::start_new();
            let def_id = each_mir.to_def_id();
            let body = mir_body(tcx, def_id);

            if body.basic_blocks.is_cfg_cyclic() { continue; }

            let mut cfg = z3::Config::new();
            cfg.set_model_generation(true);
            cfg.set_timeout_msec(1000);

            //let ctx = z3::Context::new(&cfg);
            //let goal = z3::Goal::new(&ctx, true, false, false);
            //let solver = z3::Solver::new(&ctx);
            //let inter_visitor = InterFlowAnalysis::new(self.rcx);

        }
    }
}
