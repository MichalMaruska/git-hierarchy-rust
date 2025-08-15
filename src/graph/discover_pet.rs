use discover_graph::GraphProvider;


// Example with external data source
pub struct ExternalDataProvider {
    call_count: usize,
}

// hardcoded to use String as
impl ExternalDataProvider {
    pub fn new() -> Self {
        Self { call_count: 0 }
    }

    fn fetch_neighbors(&mut self, vertex: &String) -> Vec<String> {
        self.call_count += 1;
        println!("API call #{}: fetching neighbors for '{}'", self.call_count, vertex);

        match vertex.as_str() {
            "root" => vec!["A".to_string(), "B".to_string(), "C".to_string()],
            "A" => vec!["A1".to_string(), "A2".to_string()],
            "B" => vec!["B1".to_string(), "B2".to_string(), "B3".to_string()],
            "C" => vec!["C1".to_string()],
            "A1" => vec!["A1a".to_string()],
            "A2" => vec!["A2a".to_string(), "A2b".to_string()],
            _ => Vec::new(),
        }
    }
}

impl GraphProvider<String> for ExternalDataProvider {
    fn get_neighbors(&mut self, vertex: &String) -> Vec<String> {
        std::thread::sleep(std::time::Duration::from_millis(10));
        self.fetch_neighbors(vertex)
    }

    fn vertex_exists(&mut self, vertex: &String) -> bool {
        !vertex.is_empty() && vertex.len() <= 10
    }
}
