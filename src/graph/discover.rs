use crate::graph::Graph;

pub trait NodeExpander {
    fn node_identity(&self) -> &str; // same lifetime

    // fixme:
    fn node_children(&self) -> Vec<Box<dyn NodeExpander>>; // owned!
}
