use rustc_middle::ty::TyCtxt;
use rustc_span::symbol::Symbol;
use rustc_span::def_id::DefId;
use rustc_span::{FileName, FileNameDisplayPreference};

pub fn get_fn_name(tcx: TyCtxt<'_>, def_id: DefId) -> Option<Symbol> {
    if def_id.is_local() {
        if let Some(node) = tcx.hir().get_if_local(def_id) {
	    match node {
		rustc_hir::Node::Item(item) => {
                    return Some(item.ident.name);
                },
	        rustc_hir::Node::ImplItem(item) => {
                    return Some(item.ident.name);
		},
		_ => { return None },
            }
        }
    }
    None
}

pub fn get_filename(tcx: TyCtxt<'_>, def_id: DefId) -> Option<String> {
    // Get the HIR node corresponding to the DefId
    if let Some(local_id) = def_id.as_local() {
	let hir_id = tcx.hir().local_def_id_to_hir_id(local_id);
        let span = tcx.hir().span(hir_id);
        let source_map = tcx.sess.source_map();

        // Retrieve the file name
        if let Some(filename) = source_map.span_to_filename(span).into() {
            return Some(convert_filename(filename));
        }
    }

    None
}

fn convert_filename(filename: FileName) -> String {
    match filename {
        FileName::Real(path) => path.to_string_lossy(FileNameDisplayPreference::Local).into_owned(),
        _ => "<unknown>".to_string(),
    }
}
