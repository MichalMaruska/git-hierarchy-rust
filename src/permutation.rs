
// we have a vector of elements
// we have an order (permutation)   5 6 3....
// which means the permutation   a[i] -> i
// and we want to apply this permutation by swap()

pub fn reorder_by_permutation<T>(vec: &mut Vec<T>, permutation: &[usize]) {
    assert_eq!(vec.len(), permutation.len(), "Vector and permutation must have the same length");

    let mut visited = vec![false; vec.len()];

    for start in 0..vec.len() {
        if visited[start] {
            continue;
        }

        // For each cycle, we need to rotate elements
        // If we have cycle a -> b -> c -> a, we do: swap(a,b), swap(a,c)
        let mut current = start;
        let mut next = permutation[current];

        while next != start {
            vec.swap(current, next);
            visited[current] = true;
            current = next;
            next = permutation[current];
        }
        visited[current] = true;
    }
}
