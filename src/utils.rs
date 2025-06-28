
pub fn concatenate(prefix: &str, suffix: &str) -> String {
    let mut s = String::from(prefix);
    s.push_str(suffix);
    s
}

pub fn extract_name(refname: &str) -> &str {
    let mut a = refname.strip_prefix("ref: ").unwrap_or(refname);
    a = a.strip_prefix("refs/").unwrap_or(a);
    a = a.strip_prefix("heads/").unwrap_or(a);
    return a;
}
