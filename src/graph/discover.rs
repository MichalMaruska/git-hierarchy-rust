use log::{self,info};
use std::collections::HashMap;

use crate::graph::Graph;

pub trait NodeExpander {
    fn node_identity(&self) -> &str; // same lifetime

    // not object-safe:
    // so Self is not ok, but NodeExpander is ?
    fn node_prepare(&mut self); // -> &dyn NodeExpander; // upgrade itself? or what
    fn node_children(&self) -> Vec<Box<dyn NodeExpander>>; // owned!
}

pub fn discover_graph(mut start: Vec<Box<dyn NodeExpander>>) -> (Graph, Vec<Box<dyn NodeExpander>>)
{
    let mut graph = Graph::new();
    graph.add_vertices(start.len());

    // this will be produced, we could use the start Vector! todo!
    let mut vertices : Vec<Box<dyn NodeExpander>> = Vec::new();

    // |start|.....
    // |------|-------------|......|  vertices
    //        ^ reader      ^appender
    //
    vertices.append(&mut start);

    /*
    vertices.reserve(start.len());
    start.into_iter().map(|x| vertices.push(x)); // move
     */

    // mmc: it's a set!  so maps into the vector indices
    let mut known : HashMap<String, usize> = HashMap::new();

    let mut current = 0;
    loop {
        let this = vertices.get_mut(current).unwrap();
        info!("visiting node {} {}", current, this.node_identity());

        this.node_prepare();

        let children =  this.node_children();

        for child in children {
            if let Some(found) = known.get(child.node_identity()) {
                info!("adding edge to already known node {}", child.node_identity());
                graph.add_edge(current, *found);
            } else {
                vertices.push(child);
                let new_index = vertices.len() - 1;
                graph.add_vertices(new_index);
                graph.add_edge(current, new_index);
                info!("adding unknown child to the list {} {}", &vertices[new_index].node_identity(), new_index);

                known.insert(vertices[new_index].node_identity().to_string(), new_index);
            }
        }

        current += 1;
        if current == vertices.len() {break}
    }

    return (graph, vertices);
}
