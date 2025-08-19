use discover_graph::GraphProvider;

use crate::git_hierarchy::{GitHierarchy};
use git2::{Repository};

// Example with external data source
pub struct GitHierarchyProvider<'repo> {
    repository: &'repo Repository,
    call_count: usize,
}

// What kind of structure do I want at the end?  .... (why?) topological order ... iterator? lookup? walk down?
// what operations do I want .... on the graph still?  Stitch/clone/replace nodes?



// can we have a hash  str () -> githierarchy?
//
// then the graph will handle str.

// Example with external data source

// hardcoded to use String as
impl<'repo>  GitHierarchyProvider<'repo> {
    pub fn new(repo: &'repo Repository) -> Self {
        Self {
            repository: repo,
            call_count: 0
        }
    }

    fn fetch_neighbors(&mut self, vertex: &String) -> Vec<String> {
        self.call_count += 1;
        println!("API call #{}: fetching neighbors for '{}'", self.call_count, vertex);
        // get from the object_map
        let repository = self.repository;

        // convert if necessary

        // Get the children,

        // put in the object_map

        // return as Strings
        return Vec::new();
    }
}

impl<'repo> GraphProvider<String> for GitHierarchyProvider<'repo> {
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
