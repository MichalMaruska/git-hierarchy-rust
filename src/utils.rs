use std::collections::HashSet;
use std::hash::Hash;
use tracing_subscriber;

pub fn concatenate(prefix: &str, suffix: &str) -> String {
    let mut s = String::from(prefix);
    s.push_str(suffix);
    s
}

pub fn extract_name(refname: &str) -> &str {
    let mut a = refname.strip_prefix("ref: ").unwrap_or(refname);
    a = a.strip_prefix("refs/").unwrap_or(a);
    a = a.strip_prefix("heads/").unwrap_or(a);
    a
}

pub fn divide_str(s: &'_ str, split_char: char) -> (&'_ str, &'_ str) {
    let v: Vec<&str> = s.split(split_char).take(2).collect();

    (v[0], v[1])
}

// todo: use hash_fn2 and use identity if necessary
/// Find elements in iter2 that are not equal to the hash function output of iter1
///
/// # Arguments
/// * `iter1` - First iterator whose elements will be passed through the hash function
/// * `iter2` - Second iterator whose elements will be compared against hashed iter1 elements
/// * `hash_fn` - Function that takes an element from iter1 and returns a hashable value
///
/// # Returns
/// Vector of elements from iter2 that don't match any hash function output from iter1
pub fn find_non_matching_elements<I1, I2, T1, T2, F>(iter1: I1, iter2: I2, hash_fn: F) -> Vec<T2>
where
    I1: IntoIterator<Item = T1>,
    I2: IntoIterator<Item = T2>, // mmc: is T2 & H the same?
    T2: Clone + Hash + Eq,    // T2 comparable with H todo: or Borrow<H> ?
// PartialEq<H>
    // H: Hash + Eq, H,
    F: Fn(T1) -> T2, // H
{
    // mmc: so  iter2 - hash(iter1)

    // Apply hash function to all elements in iter1 and collect into a HashSet
    let hashed_set: HashSet<T2> = iter1.into_iter().map(hash_fn).collect();

    // Filter iter2 to find elements that don't match any hashed value
    iter2
        .into_iter()
        .filter(|item| // !hashed_set.iter().any(|hashed| item == hashed
                hashed_set.contains(item)
                // iter().any(|hashed| item == hashed)
        )
        .collect()
}

pub fn init_tracing(verbose: u8) {
    if verbose > 0 {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_concatenate() {
        assert_eq!("Hello World", concatenate("Hello ", "World"));
    }

    #[test]
    fn test_divide_str() {
        assert_eq!(divide_str("Hello World", ' '), ("Hello", "World"));
    }

    #[test]
    fn test_extract_name() {
        assert_eq!("name", extract_name("name"));
        assert_eq!("name", extract_name("heads/name"));
        assert_eq!("name", extract_name("refs/heads/name"));
    }

    #[test]
    fn test_find_non_matching_elements() {
        let real = vec![1, 2, 10, 16];
        let selected = vec![0, 2, 5, 6];

        let found = find_non_matching_elements(
            real.iter(),
            selected.iter(),
            // on iter1!
            |x| x);
        // found is only &
        assert_eq!(
            &found,
            // not found in first, which *are* in 2nd
            &[&0,&5,&6]
        )
    }
}
