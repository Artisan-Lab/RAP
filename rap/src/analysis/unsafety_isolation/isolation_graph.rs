use rustc_hir::def_id::DefId;

#[derive(Debug, Clone)]
pub struct IsolationGraphNode {
    pub node_id: DefId,
    //0:constructor, 1:method, 2:function
    pub node_type: usize,
    //if this node is a method, then it may have constructors
    pub constructor_id : Option<Vec<DefId>>,
    pub node_name: String,
    pub node_unsafety: bool,
    //record all unsafe callees
    pub callees: Vec<DefId>,
    //tag if this node has been visited for its unsafe callees
    pub visited_tag: bool,
    //record the source of the func
    pub is_crate_api: bool,
}

impl IsolationGraphNode{
    pub fn new(node_id:DefId, node_type:usize, node_name: String, node_unsafety: bool, is_crate_api: bool) -> Self{
        Self {
            node_id,
            node_type,
            constructor_id: None,
            node_name,
            node_unsafety,
            callees: Vec::new(),
            visited_tag: false,
            is_crate_api,
        }
    }
}
