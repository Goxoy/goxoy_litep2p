use log::{debug, error, trace};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::File,
    io::{BufWriter, Write},
    sync::{Arc, Mutex},
    usize,
};

use crate::helper::{self, client_async};

pub trait VecStringArcExtension {
    fn insert_if_not_exist(&self, new_data: String) -> bool;
}
impl VecStringArcExtension for Arc<Mutex<Vec<String>>> {
    fn insert_if_not_exist(&self, new_item: String) -> bool {
        let mut founded = false;
        for item in self.lock().unwrap().iter() {
            if item.eq(&new_item.clone()) {
                founded = true;
            }
        }
        if founded == false {
            self.lock().unwrap().push(new_item);
            true
        } else {
            false
        }
    }
}

pub trait NodeDetailsExtension {
    fn check_status_for_ping(&self, ping_time_diff: u128) -> (bool, u128);
}
impl NodeDetailsExtension for NodeDetails {
    fn check_status_for_ping(&self, ping_time_diff: u128) -> (bool, u128) {
        let s_clone = self.clone();

        if s_clone.status == NodeStatus::Unknown {
            debug!("{}   => {}", s_clone.addr.clone(), "testing unknown");
            return (true, 0);
        }

        if s_clone.status == NodeStatus::Online {
            if self
                .last_access_time
                .abs_diff(helper::get_sys_time_in_millis())
                > ping_time_diff
            {
                trace!(
                    "{}   => {} [{}]",
                    self.addr.clone(),
                    "testing ONLINE",
                    ping_time_diff
                );
                return (true, ping_time_diff);
            }
            return (false, ping_time_diff);
        }

        if s_clone.status == NodeStatus::Offline {
            if self.testing_count > 2 {
                println!("offline - count > 2");
                return (false, ping_time_diff);
            }

            if helper::get_sys_time_in_secs().abs_diff(self.last_access_time) > 10 {
                debug!(
                    "{}   => {} [{}]",
                    self.addr.clone(),
                    "testing offline",
                    self.testing_count
                );
                return (false, 0);
            }
            println!("offline - FALSE");
            return (false, ping_time_diff);
        }

        (false, ping_time_diff)
    }
}
pub trait NodeDetailsArcExtension {
    fn remove_from_list(&self, node_addr: String) -> bool;
    fn store_to_disk(&self, my_node_addr: String, offline_node_list: Vec<String>);
    fn warn_the_others_about_offline_node(&self, my_node_addr: String, offline_node_addr: String);
    fn set_last_access_time(&self, node_addr: String, new_last_access_time: u128) -> bool;
    fn calculate_hash(&self) -> String;
    fn set_sync_time(&mut self, which_node: Vec<String>, new_sync_time: u128);
    fn send_state_to_all(&self, my_addr: String, state: StateType);
    fn to_node_list(&self) -> Vec<String>;
    fn set_state(&self, node_addr: String, new_state: NodeStatus);
    fn set_error_count(&self, node_addr: String, count: usize) -> bool;
    fn online_node_count(&self) -> usize;
}

impl NodeDetailsArcExtension for Arc<Mutex<Vec<NodeDetails>>> {
    fn remove_from_list(&self, node_addr: String) -> bool {
        let mut item_index = usize::MAX;
        for (n_index, n_info) in self.lock().unwrap().iter().enumerate() {
            if item_index != usize::MAX {
                continue;
            }
            if n_info.addr.eq(&node_addr.clone()) {
                item_index = n_index;
            }
        }
        if item_index == usize::MAX {
            false
        } else {
            self.lock().unwrap().remove(item_index);
            true
        }
    }
    fn store_to_disk(&self, my_node_addr: String, offline_node_list: Vec<String>) {
        let mut node_list = self.to_node_list();
        for addr_1 in offline_node_list.iter() {
            let mut founded = false;
            for addr_2 in node_list.iter() {
                if addr_1.eq(addr_2) {
                    founded = true;
                }
            }
            if founded == false {
                node_list.push(addr_1.clone());
            }
        }
        node_list.sort();

        match File::create(format!(
            "{}.json",
            my_node_addr.replace(".", "_").replace(":", "_")
        )) {
            Ok(file) => {
                let mut writer = BufWriter::new(file);
                _ = serde_json::to_writer(&mut writer, &node_list);
                _ = writer.flush();
            }
            Err(_) => {}
        }
    }
    fn warn_the_others_about_offline_node(&self, my_node_addr: String, offline_node_addr: String) {
        let node_list = self.to_node_list();
        let state_msg_vec = Message {
            id: helper::get_sys_time_in_nano(),
            sender: my_node_addr.clone(),
            kind: MessageKind::State,
            payload: StateType::ControlNodeStatus(offline_node_addr.clone()).to_byte_array(),
        }
        .to_byte_array();

        for node_addr in node_list.iter() {
            if node_addr.eq(&offline_node_addr.clone()) == false
                && node_addr.eq(&my_node_addr.clone()) == false
            {
                client_async(node_addr.clone(), state_msg_vec.clone());
            }
        }
    }

    fn set_error_count(&self, node_addr: String, count: usize) -> bool {
        for node_info in self.lock().unwrap().iter_mut() {
            if node_info.addr.eq(&node_addr) {
                node_info.testing_count = count;
                return true;
            }
        }
        return true;
    }
    fn set_last_access_time(&self, node_addr: String, new_last_access_time: u128) -> bool {
        for node_info in self.lock().unwrap().iter_mut() {
            if node_info.addr.eq(&node_addr) {
                node_info.last_access_time = new_last_access_time;
                return true;
            }
        }
        return true;
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
        let node_list = self.to_node_list();
        let msg_array = Message {
            id: helper::get_sys_time_in_nano(),
            sender: my_node_addr.clone(),
            kind: MessageKind::State,
            payload: state.to_byte_array(),
        }
        .to_byte_array();
        for receiver_addr in node_list.iter() {
            if my_node_addr.eq(receiver_addr) == false {
                let result = helper::client(receiver_addr.to_string(), msg_array.clone());
                if result.id == 0 {
                    self.set_state(receiver_addr.clone(), NodeStatus::Online);
                } else {
                    self.set_state(receiver_addr.clone(), NodeStatus::Offline);
                }
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

    fn set_state(&self, node_addr: String, new_status: NodeStatus) {
        for n_info in self.lock().unwrap().iter_mut() {
            if n_info.addr.eq(&node_addr.clone()) && n_info.status != new_status.clone() {
                n_info.status = new_status.clone();
            }
        }
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
    pub testing_count: usize,
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
