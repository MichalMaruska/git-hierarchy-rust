
pub mod discover;
use std::vec;


type Range = usize;
pub struct Graph {
    //
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
    }

    pub fn add_edge(&mut self, from: Range, to: Range) {
        // Index::index_mut(self.adjacency_list,from);
        let list = &mut self.adjacency_list;

        if (list.len() <= from) {
            list.resize(from + 1, Vec::new());
            list.resize_with(from + 1, || Vec::new());
        }

        // list.get(from);
        list[from].push(to);
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
