use log::error;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::File,
    io::{BufWriter, Write},
    sync::{Arc, Mutex},
};

use crate::helper;

pub trait NodeDetailsToHelper {
    fn store_to_disk(&self, my_node_addr: String);
    fn calculate_hash(&self) -> String;
    fn set_sync_time(&mut self, which_node: Vec<String>, new_sync_time: u128);
    fn send_state_to_all(&self, my_addr: String, state: StateType);
    fn to_node_list(&self) -> Vec<String>;
    fn online_node_count(&self) -> usize;
}
impl NodeDetailsToHelper for Arc<Mutex<Vec<NodeDetails>>> {
    fn store_to_disk(&self, my_node_addr: String) {
        match File::create(format!(
            "{}.json",
            my_node_addr.replace(".", "_").replace(":", "_")
        )) {
            Ok(file) => {
                let mut writer = BufWriter::new(file);
                _ = serde_json::to_writer(&mut writer, &self.to_node_list());
                _ = writer.flush();
            }
            Err(_) => {}
        }
    }
    fn calculate_hash(&self) -> String {
        let mut tmp_status = Vec::new();
        let mut tmp_list = Vec::new();
        for n_info in self.lock().unwrap().clone().iter() {
            tmp_list.push(n_info.addr.clone());
            tmp_status.push(format!("{}:{}", n_info.addr.clone(), n_info.status));
        }
        tmp_list.sort();
        tmp_status.sort();

        let list_hash = format!(
            "{:x}",
            md5::compute(serde_json::to_string(&tmp_list).unwrap())
        );
        let status_hash = format!(
            "{:x}",
            md5::compute(serde_json::to_string(&tmp_status).unwrap())
        );
        format!("{}:{}", &list_hash[..5], &status_hash[..5])
    }
    fn set_sync_time(&mut self, which_node: Vec<String>, new_sync_time: u128) {
        for n_info in self.lock().unwrap().iter_mut() {
            if which_node.len() > 0 {
                for n_addr in which_node.iter() {
                    if n_addr.eq(&n_info.addr.clone()) {
                        n_info.synced_time_as_secs = new_sync_time;
                    }
                }
            } else {
                n_info.synced_time_as_secs = new_sync_time;
            }
        }
    }
    fn send_state_to_all(&self, my_node_addr: String, state: StateType) {
        let msg_array = Message {
            id: helper::get_sys_time_in_nano(),
            sender: my_node_addr.clone(),
            kind: MessageKind::State,
            payload: state.to_byte_array(),
        }
        .to_byte_array();
        for receiver_addr in self.to_node_list().iter() {
            if my_node_addr.eq(receiver_addr) == false {
                _ = helper::client(receiver_addr.to_string(), msg_array.clone());
            }
        }
    }

    fn to_node_list(&self) -> Vec<String> {
        let mut tmp_node_list = Vec::new();
        for n_info in self.lock().unwrap().clone().iter() {
            tmp_node_list.push(n_info.addr.clone());
        }
        tmp_node_list.sort();
        tmp_node_list
    }

    fn online_node_count(&self) -> usize {
        let mut r_count = 0;
        for n_info in self.lock().unwrap().clone().iter() {
            if n_info.status == NodeStatus::Online {
                r_count = r_count + 1;
            }
        }
        r_count
    }
}

pub trait ConvertVecToStruct {
    fn to_message_struct(&self) -> Message;
    fn to_state_struct(&self) -> StateType;
}
impl ConvertVecToStruct for Vec<u8> {
    fn to_message_struct(&self) -> Message {
        match String::from_utf8(self.clone()) {
            Ok(income) => {
                let payload_result: serde_json::Result<Message> = serde_json::from_str(&income);
                if payload_result.is_ok() {
                    return payload_result.unwrap();
                }
                error!("Message convert error [ 8394 ]");
            }
            Err(_) => {
                error!("Message convert error [ 2039 ]");
            }
        }
        Message {
            id: 0,
            sender: String::new(),
            kind: MessageKind::Error,
            payload: Vec::new(),
        }
    }
    fn to_state_struct(&self) -> StateType {
        match String::from_utf8(self.clone()) {
            Ok(income) => {
                let payload_result: serde_json::Result<StateType> = serde_json::from_str(&income);
                if payload_result.is_ok() {
                    return payload_result.unwrap();
                }
                error!("Message convert error [ 8394 ]");
            }
            Err(_) => {
                error!("Message convert error [ 2039 ]");
            }
        }
        StateType::Unknown()
    }
}

impl Message {
    pub fn to_byte_array(&self) -> Vec<u8> {
        match serde_json::to_string(&self.clone()) {
            Ok(result) => result.as_bytes().to_vec(),
            Err(_) => {
                error!("Message convert error [ 4827 ]");
                Vec::new()
            }
        }
    }
}

impl StateType {
    pub fn to_byte_array(&self) -> Vec<u8> {
        match serde_json::to_string(&self.clone()) {
            Ok(result) => result.as_bytes().to_vec(),
            Err(_) => {
                error!("Message convert error [ 4827 ]");
                Vec::new()
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Message {
    pub id: u128,
    pub sender: String,
    pub kind: MessageKind,
    pub payload: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessageKind {
    Ok,

    // node'ların arasında özel mesaj gönderi için kullanılan mesaj türü
    State,

    // dağıtılacak mesaj
    Distribute,

    // hatalı mesaj veya işlem tipi
    Error,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeDetails {
    pub addr: String,
    pub node_hash: String,
    pub last_access_time: u128,
    pub synced_time_as_secs: u128,
    pub status: NodeStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeStatus {
    Online,
    Offline,
    Unknown,
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl fmt::Display for MessageKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StateType {
    Unknown(),
    Ping(String),
    ControlNodeStatus(String),
    NodeList(Vec<String>),
}
