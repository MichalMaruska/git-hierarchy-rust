use discover_graph::GraphProvider;

use crate::git_hierarchy::{GitHierarchy,load};

// GitHierarchy implements this, but we have to import explicitly.
// ^^^^^^^^^^^^^ method not found in `GitHierarchy<'_>`
use crate::graph::discover::NodeExpander;

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
        // get from the object_map
        let repository = self.repository;
        let gh = load(repository, vertex).unwrap();
        // convert if necessary

        // Get the children,
        let mut ch = Vec::new();

        match gh {
            // regular branch. say `master'
            GitHierarchy::Name(_x) => {panic!("unprepared")}
            GitHierarchy::Segment(s) => {
                let symbolic_base = s.base(repository);
                // back to name...
                ch.push(symbolic_base.name().unwrap().to_owned());
            }
            GitHierarchy::Sum(s) => {
                // copy
                for summand in s.summands(&repository) {
                    ch.push(summand.node_identity().to_owned());
                }
            }
            GitHierarchy::Reference(_r) => {
                // Vec::new()
            }
        }

        // let ch = gh.node_children();
        // this should be Strings

        // put in the object_map

        // return as Strings
        // convert vec<&str> to vec<String> ?
        return ch;
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
