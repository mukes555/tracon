/// Package-manager install invocations we recognize inside shell commands.
/// Order matters: longer, more specific patterns first so "uv pip install"
/// wins over "pip install".
const INSTALL_PATTERNS: &[&str] = &[
    "uv pip install",
    "npm install",
    "npm i ",
    "pnpm install",
    "pnpm add",
    "yarn add",
    "bun install",
    "bun add",
    "pip3 install",
    "pip install",
    "uv add",
    "poetry add",
    "cargo install",
    "cargo add",
    "gem install",
    "go install",
    "brew install",
    "winget install",
    "choco install",
    "apt-get install",
    "apt install",
    "dnf install",
    "yum install",
    "snap install",
    "mas install",
    "pipx install",
    // npx downloads and executes a package in one step: install AND run.
    "npx ",
];

/// If the command contains a package install, return the install invocation
/// (from the manager verb onward) for the timeline. Compound commands like
/// "cd api && npm install express" still match.
pub fn detect_package_install(command: &str) -> Option<String> {
    for pattern in INSTALL_PATTERNS {
        let Some(idx) = command.find(pattern) else {
            continue;
        };
        if !starts_a_word(command, idx) {
            continue;
        }
        let snippet: String = command[idx..].chars().take(200).collect();
        return Some(snippet.trim_end().to_string());
    }
    None
}

/// One package reference extracted from an install invocation, in the shape
/// the OSV API and registries speak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSpec {
    /// OSV ecosystem name: "npm", "PyPI", "crates.io", "RubyGems", "Go".
    pub ecosystem: String,
    pub name: String,
    pub version: Option<String>,
}

/// Parse the packages named in an install invocation (the string returned by
/// detect_package_install). Bare installs from a lockfile ("npm install")
/// yield an empty list. Flags and shell noise are skipped; unknown managers
/// (brew) yield nothing.
pub fn parse_specs(install: &str) -> Vec<PackageSpec> {
    let mut words = install.split_whitespace();
    let manager = words.next().unwrap_or("");
    let Some(ecosystem) = ecosystem_for(manager) else {
        return Vec::new();
    };

    words
        .skip_while(|w| is_subcommand_word(w))
        .take_while(|w| !is_shell_operator(w))
        .filter(|w| !w.starts_with('-'))
        .filter_map(|w| spec_from_token(ecosystem, w))
        .collect()
}

fn ecosystem_for(manager: &str) -> Option<&'static str> {
    match manager {
        "npm" | "pnpm" | "yarn" | "bun" => Some("npm"),
        "pip" | "pip3" | "uv" | "poetry" => Some("PyPI"),
        "cargo" => Some("crates.io"),
        "gem" => Some("RubyGems"),
        "go" => Some("Go"),
        _ => None,
    }
}

/// Words like "install", "add", or uv's "pip" that sit between the manager
/// and the first package name.
fn is_subcommand_word(word: &str) -> bool {
    matches!(word, "install" | "i" | "add" | "pip")
}

fn is_shell_operator(word: &str) -> bool {
    matches!(word, "&&" | "||" | "|" | ";" | ">" | ">>" | "2>&1")
        || word.starts_with("2>")
        || word.starts_with('>')
}

fn spec_from_token(ecosystem: &str, token: &str) -> Option<PackageSpec> {
    let cleaned = token.trim_matches(|c| c == '"' || c == '\'');
    if cleaned.is_empty() || cleaned.contains('/') && ecosystem != "npm" && ecosystem != "Go" {
        return None;
    }

    // npm scoped packages keep their leading @; a version comes after a later @.
    if ecosystem == "npm" {
        let at = if let Some(rest) = cleaned.strip_prefix('@') {
            rest.find('@').map(|i| i + 1)
        } else {
            cleaned.find('@')
        };
        return match at {
            Some(idx) => Some(PackageSpec {
                ecosystem: ecosystem.into(),
                name: cleaned[..idx].into(),
                version: Some(cleaned[idx + 1..].into()),
            }),
            None => Some(PackageSpec {
                ecosystem: ecosystem.into(),
                name: cleaned.into(),
                version: None,
            }),
        };
    }

    // PyPI pins use ==; other separators (>=, <=, ~=) aren't exact versions.
    if let Some((name, version)) = cleaned.split_once("==") {
        return Some(PackageSpec {
            ecosystem: ecosystem.into(),
            name: name.into(),
            version: Some(version.into()),
        });
    }
    let name = cleaned
        .split(['>', '<', '~', '['])
        .next()
        .unwrap_or(cleaned);
    if name.is_empty() {
        return None;
    }
    Some(PackageSpec {
        ecosystem: ecosystem.into(),
        name: name.into(),
        version: None,
    })
}

/// The match must begin a command word, not sit inside one ("mynpm install").
fn starts_a_word(command: &str, idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    let before = command[..idx].chars().last().unwrap_or(' ');
    matches!(before, ' ' | '\t' | '\n' | ';' | '&' | '|' | '(')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_plain_npm_install() {
        assert_eq!(
            detect_package_install("npm install express jsonwebtoken").as_deref(),
            Some("npm install express jsonwebtoken")
        );
    }

    #[test]
    fn detects_install_inside_compound_command() {
        assert_eq!(
            detect_package_install("cd api && pnpm add zod").as_deref(),
            Some("pnpm add zod")
        );
    }

    #[test]
    fn prefers_uv_pip_over_pip() {
        assert_eq!(
            detect_package_install("uv pip install requests").as_deref(),
            Some("uv pip install requests")
        );
    }

    #[test]
    fn ignores_non_install_commands() {
        assert!(detect_package_install("npm run build").is_none());
        assert!(detect_package_install("cargo build --release").is_none());
        assert!(detect_package_install("mynpm installx").is_none());
    }

    #[test]
    fn detects_system_and_app_managers() {
        assert!(detect_package_install("brew install --cask figma").is_some());
        assert!(detect_package_install("winget install Figma.Figma").is_some());
        assert!(detect_package_install("sudo apt-get install ripgrep").is_some());
        assert!(detect_package_install("snap install code --classic").is_some());
        assert_eq!(
            detect_package_install("npx create-react-app my-app").as_deref(),
            Some("npx create-react-app my-app")
        );
    }

    #[test]
    fn parses_npm_specs_with_scopes_and_versions() {
        let specs = parse_specs("pnpm add @sentry/react@10.69.0 zod --save-exact");
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].name, "@sentry/react");
        assert_eq!(specs[0].version.as_deref(), Some("10.69.0"));
        assert_eq!(specs[0].ecosystem, "npm");
        assert_eq!(specs[1].name, "zod");
        assert_eq!(specs[1].version, None);
    }

    #[test]
    fn parses_pip_specs_and_stops_at_shell_operators() {
        let specs = parse_specs("pip install requests==2.31.0 flask 2>&1 && echo done");
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].name, "requests");
        assert_eq!(specs[0].version.as_deref(), Some("2.31.0"));
        assert_eq!(specs[0].ecosystem, "PyPI");
        assert_eq!(specs[1].name, "flask");
    }

    #[test]
    fn uv_pip_install_maps_to_pypi() {
        let specs = parse_specs("uv pip install httpx");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].ecosystem, "PyPI");
        assert_eq!(specs[0].name, "httpx");
    }

    #[test]
    fn bare_lockfile_install_yields_no_specs() {
        assert!(parse_specs("pnpm install --frozen-lockfile").is_empty());
        assert!(parse_specs("npm install").is_empty());
    }
}
