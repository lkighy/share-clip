use std::path::{Component, Path};

pub fn validate_relative_path(p: &str) -> bool {
    let path = Path::new(p);

    for comp in path.components() {
        match comp {
            Component::ParentDir => return false,
            Component::RootDir => return false,
            Component::Prefix(_) => return false,
            _ => {}
        }
    }

    true
}
