extern crate serde;
extern crate toml;

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub name: String,
    pub pointcuts: Vec<PointCut>,
}

#[derive(Deserialize, Debug)]
pub struct PointCut {
    pub condition: String,
    pub advice: String,
}

pub fn get_root() -> Result<PathBuf, String> {
    let root = std::env::current_dir().map_err(|e| format!("{}", e))?;
    if !root.join("Cargo.toml").is_file() {
        return Err(format!("`{:?}` does not look like a Rust/Cargo project", root));
    }
    Ok(root)
}

pub fn parse_config() -> Config {
    let mut cur_proj = get_root().expect("failed to found root folder");
    cur_proj.push("Aspect.toml");
    let content = std::fs::read(cur_proj).unwrap();
    let s = String::from_utf8_lossy(&content);
    toml::from_str(s.as_ref()).unwrap()
}
