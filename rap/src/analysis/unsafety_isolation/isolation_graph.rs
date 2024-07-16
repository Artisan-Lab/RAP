use rustc_hir::def_id::DefId;

#[derive(Debug, Clone)]
pub struct IsolationGraphNode {
    pub node_id: DefId,
    //0:constructor, 1:method, 2:function
    pub node_type: usize,
    pub node_name: String,
    pub node_unsafety: bool,
    //if this node is a method, then it may have constructors
    pub constructors : Vec<DefId>,
    //record all unsafe callees
    pub callees: Vec<DefId>,
    //tag if this node has been visited for its unsafe callees
    pub methods: Vec<DefId>,
    pub callers: Vec<DefId>,
    pub visited_tag: bool,
    //record the source of the func
    pub is_crate_api: bool,
}

impl IsolationGraphNode{
    pub fn new(node_id:DefId, node_type:usize, node_name: String, node_unsafety: bool, is_crate_api: bool) -> Self{
        Self {
            node_id,
            node_type,
            node_name,
            node_unsafety,
            constructors: Vec::new(),
            callees: Vec::new(),
            methods: Vec::new(),
            callers: Vec::new(),
            visited_tag: false,
            is_crate_api,
        }
    }
}
