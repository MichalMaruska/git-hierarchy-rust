pub mod discover;
pub mod discover_pet;
pub mod topology_sort;
use crate::graph::topology_sort::topological_sort;

type Range = usize;
pub struct Graph {
    vertices: usize,
    adjacency_list: Vec<Vec<Range>>,
}

impl Graph {
    pub fn new() -> Self {
        Graph {
            vertices: 0,
            adjacency_list: Vec::new(),
        }
    }
    pub fn add_vertices(&mut self, n: usize) {
        if n > self.vertices {
            self.vertices = n;
        }

        // reserve:
        let list = &mut self.adjacency_list;
        if list.len() <= n {
            list.resize(n + 1, Vec::new());
            list.resize_with(n + 1, || Vec::new());
        }
    }

    pub fn add_edge(&mut self, from: Range, to: Range) {
        // Index::index_mut(self.adjacency_list,from);
        let list = &mut self.adjacency_list;

        // list.get(from);
        list[from].push(to);
    }

    pub fn toposort(&self) -> Vec<usize> {
        let matrix = &self.adjacency_list;

        if let Some(order) = topological_sort(matrix) {
            // println!("found order {:?}", order);
            order
        } else {
            panic!("bad topo order");
        }
    }

    pub fn dump_graph(&self) {
        println!("Graph of {} vertices:", self.vertices);
        let matrix = &self.adjacency_list;
        for (index, row) in matrix.into_iter().enumerate() {
            print!("{}:", index);
            for edge in row {
                print!("{}", edge);
            }
            println!();
        }
    }
}
