use std::fs::File;
use std::io::Read;

use crate::domain::yml_config::YmlConfig;



pub fn load_yaml_config(path: &str) -> Result<YmlConfig, String> {
    let mut file =
        File::open(path).map_err(|e| format!("Failed to open config file {}. \n\t{}", path, e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Failed to read config file {}. \n\t{}", path, e))?;
    serde_yaml::from_str(&contents)
        .map_err(|e| format!("Failed to parse YAML config {}. \n\t{}", path, e))
}
