pub trait NodeExpander {
    fn node_identity(&self) -> &str; // same lifetime

    // not object-safe:
    // so Self is not ok, but NodeExpander is ?
    fn node_prepare(&mut self); // -> &dyn NodeExpander; // upgrade itself? or what

    // fixme:
    fn node_children(&self) -> Vec<Box<dyn NodeExpander>>; // owned!
}
