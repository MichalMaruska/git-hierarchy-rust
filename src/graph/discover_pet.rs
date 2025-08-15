use discover_graph::GraphProvider;
use crate::git_hierarchy::{GitHierarchy};

// Example with external data source
pub struct GitHierarchyProvider {
    call_count: usize,
}



// What kind of structure do I want at the end?  .... (why?) topological order ... iterator? lookup? walk down?
// what operations do I want .... on the graph still?  Stitch/clone/replace nodes?



// can we have a hash  str () -> githierarchy?
//
// then the graph will handle str.

// Example with external data source

// hardcoded to use String as
impl GitHierarchyProvider {
    pub fn new() -> Self {
        Self {
            call_count: 0
        }
    }

    fn fetch_neighbors(&mut self, vertex: &String) -> Vec<String> {
        self.call_count += 1;
        println!("API call #{}: fetching neighbors for '{}'", self.call_count, vertex);
        // get from the object_map

        // convert if necessary

        // Get the children,

        // put in the object_map

        // return as Strings
        return Vec::new();
    }
}

impl GraphProvider<String> for GitHierarchyProvider {
    fn get_neighbors(&mut self, vertex: &String) -> Vec<String> {
        std::thread::sleep(std::time::Duration::from_millis(10));
        self.fetch_neighbors(vertex)
    }

    /*
    fn vertex_exists(&mut self, vertex: &String) -> bool {
        // present in the object_map
        // !vertex.is_empty() && vertex.len() <= 10
        true
    }
    */
}
