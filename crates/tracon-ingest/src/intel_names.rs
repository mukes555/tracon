//! Package-name hygiene for the threat-intel worker.
//!
//! Names come out of shell commands an agent typed, so before one is placed
//! in a registry URL it must match the ecosystem's own grammar. Anything else
//! (path traversal, shell fragments, unicode look-alikes) is not looked up.

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

const MAX_NAME_LEN: usize = 214;

/// Everything but unreserved characters is encoded, so a scoped npm name
/// "@scope/pkg" becomes "%40scope%2Fpkg", the form registry.npmjs.org expects.
const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

pub fn is_valid_name(ecosystem: &str, name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return false;
    }
    match ecosystem {
        "npm" => is_valid_npm_name(name),
        "PyPI" => name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
        "crates.io" => name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
        "RubyGems" => name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
        "Go" => name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._/-".contains(&b)),
        "brew" => name
            .bytes()
            .all(|b| is_lower_alnum(b) || b"@._+-/".contains(&b)),
        _ => false,
    }
}

fn is_valid_npm_name(name: &str) -> bool {
    let unscoped = match name.strip_prefix('@') {
        Some(rest) => {
            let Some((scope, pkg)) = rest.split_once('/') else {
                return false;
            };
            if !is_npm_segment(scope) {
                return false;
            }
            pkg
        }
        None => name,
    };
    is_npm_segment(unscoped)
}

fn is_npm_segment(segment: &str) -> bool {
    let allowed = segment
        .bytes()
        .all(|b| is_lower_alnum(b) || b"._-".contains(&b));
    !segment.is_empty() && allowed
}

fn is_lower_alnum(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit()
}

pub fn encode_path_segment(name: &str) -> String {
    utf8_percent_encode(name, PATH_SEGMENT).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_well_formed_names_per_ecosystem() {
        assert!(is_valid_name("npm", "left-pad"));
        assert!(is_valid_name("npm", "@types/node"));
        assert!(is_valid_name("PyPI", "Django_Rest.framework"));
        assert!(is_valid_name("crates.io", "serde_json"));
        assert!(is_valid_name("Go", "github.com/spf13/cobra"));
        assert!(is_valid_name("brew", "python@3.12"));
    }

    #[test]
    fn rejects_traversal_shell_and_unknown_ecosystems() {
        assert!(!is_valid_name("npm", "../../etc/passwd"));
        assert!(!is_valid_name("npm", "@scope"));
        assert!(!is_valid_name("npm", "Left-Pad"));
        assert!(!is_valid_name("npm", "pkg;rm -rf /"));
        assert!(!is_valid_name("crates.io", "serde.json"));
        assert!(!is_valid_name("PyPI", ""));
        assert!(!is_valid_name("PyPI", &"a".repeat(300)));
        assert!(!is_valid_name("conda", "numpy"));
    }

    #[test]
    fn scoped_names_are_encoded_for_registry_paths() {
        assert_eq!(encode_path_segment("@types/node"), "%40types%2Fnode");
        assert_eq!(encode_path_segment("left-pad"), "left-pad");
    }
}
