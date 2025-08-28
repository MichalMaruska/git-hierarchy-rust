use crate::graph::Graph;

pub trait NodeExpander {
    fn node_identity(&self) -> &str; // same lifetime
}
