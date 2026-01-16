use std::collections::HashSet;
use std::hash::Hash;
use tracing_subscriber::{FmtSubscriber,self};
use std::borrow::Borrow;

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

// Return: iter2 - hash(iter1)
pub fn iterator_difference<T, U, I1, I2>(iter1: I1, iter2: I2) -> Vec<U>
where
    T: Hash + Eq,
    U: Borrow<T> + Clone,
    I1: IntoIterator<Item = T>,
    I2: IntoIterator<Item = U>,
{
    let hashed_set: HashSet<T> = iter1.into_iter().collect();

    iter2.into_iter()
        .filter(|item|
                hashed_set.contains(item.borrow()))
        .collect()
    // todo: return an iterator?
}


pub fn iterator_symmetric_difference<T, U, I1, I2>(iter1: I1, iter2: I2) -> (Vec<T>, Vec<U>)
where
    T: Hash + Eq,
    U: Borrow<T> + Clone,
    I1: IntoIterator<Item = T>,
    I2: IntoIterator<Item = U>,
{
    let mut hashed_set: HashSet<T> = iter1.into_iter().collect();
    let mut not_found = Vec::<U>::new();

    iter2.into_iter()
        .for_each(|item|
                  if hashed_set.contains(item.borrow()) {
                      hashed_set.remove(item.borrow());
                  } else {
                      not_found.push(item);
                  }
        );
    return(
        hashed_set.drain().collect(),
        not_found)
}


pub fn init_tracing(verbose: u8) {
    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        tracing::subscriber::set_global_default(
            FmtSubscriber::builder().with_env_filter(rust_log).finish(),
        ).expect("tracing setup failed");
    } else {
        if verbose > 1 {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .init();
        } else if verbose == 1 {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .init();
        } else {
            tracing_subscriber::fmt::init();
        }
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
    fn test_extract_name() {
        assert_eq!("name", extract_name("name"));
        assert_eq!("name", extract_name("heads/name"));
        assert_eq!("name", extract_name("refs/heads/name"));
    }

    #[test]
    fn test_iterator_symmetric_difference() {
        let real = vec![1, 2, 10, 16];
        let selected = vec![0, 2, 5, 6];

        let (unselected, missing) = iterator_symmetric_difference(
            real.iter(),
            selected.iter()
            );

        // found is only &
        assert_eq!(
            &unselected,
            // not found in first, which *are* in 2nd
            &[&0,&5,&6]
        );

        assert_eq!(
            &missing,
            // not found in first, which *are* in 2nd
            &[&0,&5,&6]
        )
    }
}
