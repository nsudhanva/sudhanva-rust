//! The crate version, the `VERSION` constant, the `User-Agent`, and the README
//! install instructions must agree.

use sudhanva::{USER_AGENT, VERSION};

fn manifest_version() -> String {
    let manifest = include_str!("../Cargo.toml");
    let package = manifest
        .split("[package]")
        .nth(1)
        .expect("Cargo.toml has a [package] table");
    package
        .lines()
        .find_map(|line| line.trim().strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("[package] declares a version")
        .to_owned()
}

#[test]
fn crate_version_matches_the_sdk_version() {
    assert_eq!(manifest_version(), VERSION);
    assert_eq!(USER_AGENT, format!("sudhanva-rust/{VERSION}"));
}

#[test]
fn readme_installs_the_current_release() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains(&format!("cargo add sudhanva@{VERSION}")),
        "README.md should install version {VERSION}"
    );
}
