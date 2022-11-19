use std::path::Path;

pub fn path_str(path: &Path) -> &str {
    path.to_str().unwrap()
}
