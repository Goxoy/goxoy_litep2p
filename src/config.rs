use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::io::Write;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub debug: bool,
    pub store_node_list: bool,
    pub addr: String,
    pub bootstrap: Vec<String>,
}

impl Config {
    pub fn new(config_file_name: Option<String>) -> Config {
        let file_data = std::fs::read_to_string(if config_file_name.is_some() {
            config_file_name.unwrap()
        } else {
            "p2p_config.json".to_string()
        });
        if file_data.is_ok() {
            let file_result = file_data.unwrap();
            let result_config: Config =
                serde_json::from_str(&file_result.clone()).expect("JSON was not well-formatted");
            result_config
        } else {
            let result = Config {
                debug: true,
                store_node_list: true,
                addr: "127.0.0.1:1111".to_string(),
                bootstrap: Vec::new(),
            };

            let file = std::fs::File::create("p2p_config.json".to_string());
            if file.is_ok() {
                let read_text = serde_json::to_string(&result.clone()).unwrap();
                let mut file_result = file.unwrap();
                _ = file_result.write(&read_text.as_bytes());
            }
            result
        }
    }
}
