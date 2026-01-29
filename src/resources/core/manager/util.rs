pub(crate) fn normalize_rule_path(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(raw.len());
    let mut prev_slash = false;
    for ch in raw.chars() {
        if ch == '/' {
            if !prev_slash {
                out.push('/');
                prev_slash = true;
            }
        } else {
            prev_slash = false;
            out.push(ch);
        }
    }
    if out.is_empty() { raw.to_string() } else { out }
}
