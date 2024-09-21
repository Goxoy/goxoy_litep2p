use colored::Colorize;
use config::Config;
use helper::client_async;
use log::{debug, error, info, trace};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};
use structs::{
    ConvertVecToStruct, Message, MessageKind, NodeDetails, NodeDetailsToHelper, NodeStatus,
    StateType,
};
use worker::ThreadPool;

mod config;
mod handle_connection;
mod helper;
mod structs;
mod worker;

pub struct MessageConfig {
    ping_time: u128,
}
pub struct MessagePool {
    pub my_addr: String,
    hard_config: MessageConfig,
    store_node_list_active: bool,
    node_hash: Arc<Mutex<String>>,
    node_list_synced: Arc<Mutex<String>>,
    node_hash_updated: Arc<Mutex<bool>>,
    node_status_change: Arc<Mutex<Vec<(String, NodeStatus)>>>,
    node_list: Arc<Mutex<Vec<NodeDetails>>>,
    msg_list: Arc<Mutex<Vec<Message>>>,
}

impl MessagePool {
    pub fn new() -> Self {
        env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .format_target(false)
            .format_timestamp(None)
            .init();

        MessagePool {
            my_addr: String::new(),
            hard_config: MessageConfig { ping_time: 250 },
            store_node_list_active: true,
            node_list_synced: Arc::new(Mutex::new(String::new())),
            node_hash_updated: Arc::new(Mutex::new(true)),
            node_hash: Arc::new(Mutex::new(String::new())),
            node_status_change: Arc::new(Mutex::new(Vec::new())),
            node_list: Arc::new(Mutex::new(Vec::new())),
            msg_list: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn start(&mut self, config_file_name: Option<String>) {
        let conf = Config::new(config_file_name);
        self.store_node_list_active = conf.store_node_list;

        self.my_addr = conf.addr.clone();
        self.add_node_to_list(conf.addr.clone());
        self.add_node_to_lists(conf.bootstrap.clone());
        self.add_node_to_lists(self.load_from_disk());

        info!(
            "{}     => {}",
            "STARTING NODE".bright_cyan(),
            self.my_addr.clone().to_string().bright_cyan()
        );

        self.update_node_hash();
        if self.thread_socket() == true {
            self.thread_ping();
        } else {
            error!("TCP Could Not started");
            std::process::exit(9);
        }
    }

    fn process_message(&mut self, income_msg: Message) {
        match income_msg.kind.clone() {
            MessageKind::Error => {
                // println!("Error")
            }
            MessageKind::Ok => {
                //
            }
            MessageKind::State => {
                self.process_state(income_msg);
            }
            MessageKind::Distribute => {
                debug!("distribute message");
            }
        }
    }

    fn process_state(&mut self, income_msg: Message) {
        match income_msg.payload.to_state_struct() {
            StateType::Unknown() => {
                println!("hatali state geldi");
            }
            StateType::Ping(income_node_list_hash) => {
                trace!("Ping Msg Arrived");
                self.add_node_to_list(income_msg.sender.clone());
                if self.store_node_list_active == true {
                    self.node_list.store_to_disk(self.my_addr.clone());
                }

                let new_node_hash = self.node_list.calculate_hash();
                if self.node_hash.lock().unwrap().eq(&new_node_hash) == false {
                    *self.node_hash.lock().unwrap() = new_node_hash.clone();
                }

                for item in self.node_list.lock().unwrap().iter_mut() {
                    if item.addr.eq(&income_msg.sender.clone()) {
                        item.node_hash = income_node_list_hash.clone();
                        break;
                    }
                }
                *self.node_hash.lock().unwrap() = self.node_list.calculate_hash();
            }
            StateType::ControlNodeStatus(new_status_node_addr) => {
                // println!("node durumu değişti -> {}", new_status_node_addr.clone());
                for node_info in self.node_list.lock().unwrap().iter_mut() {
                    if node_info.addr.eq(&new_status_node_addr) {
                        node_info.last_access_time = 0;
                        break;
                    }
                }
            }
            StateType::NodeList(income_node_list) => {
                trace!("Node List Arrived");
                if self.add_node_to_lists(income_node_list.clone()) == true {
                    *self.node_hash.lock().unwrap() = self.node_list.calculate_hash();
                    *self.node_hash_updated.lock().unwrap() = false;
                    if self.store_node_list_active == true {
                        self.node_list.store_to_disk(self.my_addr.clone());
                    }
                    trace!("Send Node List To =>{}", income_msg.sender.clone());
                } else {
                    trace!("Node List Equal With {}", income_msg.sender.clone());
                }
            }
        }
    }
    pub fn get_message(&mut self) -> Message {
        let income_msg = self.msg_list.lock().unwrap()[0].clone();
        self.dispose_message();
        self.process_message(income_msg.clone());
        income_msg
    }

    pub fn dispose_message(&mut self) {
        self.msg_list.lock().unwrap().remove(0);
    }

    pub fn send_to(&mut self, receiver: String, payload: Vec<u8>) {
        _ = helper::client_async(
            receiver.clone(),
            Message {
                id: helper::get_sys_time_in_nano(),
                sender: self.my_addr.clone(),
                kind: MessageKind::Distribute,
                payload: payload,
            }
            .to_byte_array(),
        );
    }
    pub fn distribute(&mut self, payload: Vec<u8>) {
        trace!("Distributing To Nodes");
        let msg_payload = Message {
            id: helper::get_sys_time_in_nano(),
            sender: self.my_addr.clone(),
            kind: MessageKind::Distribute,
            payload: payload,
        }
        .to_byte_array();

        for n_info in self.node_list.lock().unwrap().iter() {
            if n_info.addr.eq(&self.my_addr.clone()) == false {
                _ = helper::client_async(n_info.addr.to_string(), msg_payload.clone());
            }
        }
    }

    pub fn on_event(&mut self) -> EventType {
        let (node_addr, node_status) = self.status_changed();
        match node_status {
            NodeStatus::Online => {
                info!(
                    "{}            => {}",
                    "Online".bright_green(),
                    node_addr.clone()
                );
                return EventType::OnNodeStatusChanged(node_addr, NodeStatus::Online);
            }
            NodeStatus::Offline => {
                info!(
                    "{}           => {}",
                    "Offline".bright_red(),
                    node_addr.clone()
                );
                return EventType::OnNodeStatusChanged(node_addr, NodeStatus::Offline);
            }
            _ => {}
        }

        if self.node_hash_updated.lock().unwrap().clone() == true {
            *self.node_hash_updated.lock().unwrap() = false;
            let current_node_list_hash = self.node_list_synced.lock().unwrap().clone();
            if current_node_list_hash.len() > 0 {
                let n_list_hash = self.node_list_synced.lock().unwrap().clone();
                info!(
                    "{}  => {} [ {} ]",
                    "Node List Synced".bright_green(),
                    n_list_hash.clone(),
                    self.node_list.online_node_count()
                );
                return EventType::OnNodesSynced(n_list_hash.clone());
            }
        }

        let msg_kind = self.select();
        if msg_kind == MessageKind::Distribute {
            let income_msg = self.get_message();
            info!(
                "{}  => {} >> {}",
                "Message Received".bright_green(),
                income_msg.sender.clone().bright_blue(),
                match String::from_utf8(income_msg.payload.clone()) {
                    Ok(income) => income,
                    Err(_) => "".to_string(),
                }
            );
            return EventType::OnMessage(income_msg);
        }

        if msg_kind != MessageKind::Error {
            let _income_msg = self.get_message();
        }

        return EventType::OnWait();
    }
    pub fn status_changed(&mut self) -> (String, NodeStatus) {
        if self.node_status_change.lock().unwrap().len() == 0 {
            return (String::new(), NodeStatus::Unknown);
        }

        let new_node_list_hash = self.node_list.calculate_hash();
        for n_info in self.node_list.lock().unwrap().iter_mut() {
            if n_info.addr.eq(&self.my_addr.clone()) {
                n_info.node_hash = new_node_list_hash.clone();
                *self.node_hash.lock().unwrap() = new_node_list_hash.clone();
            }
        }

        let (node_addr, node_status) = self.node_status_change.lock().unwrap()[0].clone();
        self.node_status_change.lock().unwrap().remove(0);
        if node_status == NodeStatus::Online || node_status == NodeStatus::Offline {
            return (node_addr, node_status);
        }

        (String::new(), NodeStatus::Unknown)
    }

    pub fn select(&mut self) -> MessageKind {
        if self.msg_list.lock().unwrap().len() > 0 {
            let income_msg = self.msg_list.lock().unwrap()[0].clone();
            if income_msg.kind == MessageKind::Distribute || income_msg.kind == MessageKind::State {
                return income_msg.kind;
            }
        }
        MessageKind::Error
    }

    fn thread_ping(&self) {
        let my_node_addr = self.my_addr.clone();
        let node_list = self.node_list.clone();
        let node_status_change = self.node_status_change.clone();
        let current_node_hash = self.node_hash.clone();
        let my_node_hash = self.node_hash.clone();
        let node_hash_updated = self.node_hash_updated.clone();
        let node_list_synced = self.node_list_synced.clone();
        let hard_config_ping_time = self.hard_config.ping_time;
        thread::spawn(move || {
            let mut all_node_list_changed = true;
            let mut ping_time_diff = 0;
            loop {
                let mut next_ping_time_diff = hard_config_ping_time;
                let mut update_node_hash_value = false;
                let mut update_node_status = Vec::new();
                let mut update_time = Vec::new();
                let mut move_to_offline_node = usize::MAX;
                let sending_data = Message {
                    id: helper::get_sys_time_in_nano(),
                    sender: my_node_addr.clone(),
                    kind: MessageKind::State,
                    payload: StateType::Ping(current_node_hash.lock().unwrap().clone())
                        .to_byte_array(),
                }
                .to_byte_array();
                let tmp_node_list = node_list.lock().unwrap().clone();
                for (n_index, n_info) in tmp_node_list.iter().enumerate() {
                    if move_to_offline_node == usize::MAX
                        && my_node_addr.eq(&n_info.addr.clone()) == false
                    {
                        let send_ping_to_node = match n_info.status {
                            NodeStatus::Online => {
                                let curr_millis = helper::get_sys_time_in_millis();
                                let time_diff = n_info.last_access_time.abs_diff(curr_millis);
                                if time_diff > ping_time_diff {
                                    true
                                } else {
                                    false
                                }
                            }
                            NodeStatus::Offline => {
                                let current_time = helper::get_sys_time_in_secs();
                                if current_time.abs_diff(n_info.last_access_time) > 10 {
                                    // TO-DO
                                    // eğer kontrol edildiği zaman yine offline ise,
                                    // bu listeden çıkartıp offline listesine al
                                    debug!("{}   => {}", n_info.addr.clone(), "testing offline");
                                    next_ping_time_diff = 0;
                                    true
                                } else {
                                    false
                                }
                            }
                            NodeStatus::Unknown => {
                                next_ping_time_diff = 0;
                                debug!("{}   => {}", n_info.addr.clone(), "testing unknown");
                                true
                            }
                        };

                        if send_ping_to_node == true {
                            let result =
                                helper::client(n_info.addr.to_string(), sending_data.clone());
                            if result.kind == MessageKind::Ok {
                                update_time
                                    .push((n_info.addr.clone(), helper::get_sys_time_in_millis()));
                                if n_info.status != NodeStatus::Online {
                                    update_node_status
                                        .push((n_info.addr.clone(), NodeStatus::Online));
                                    node_status_change
                                        .lock()
                                        .unwrap()
                                        .push((n_info.addr.clone(), NodeStatus::Online));
                                    update_node_hash_value = true;
                                }
                            } else {
                                if result.id == 5 {
                                    update_node_hash_value = true;
                                    match n_info.status {
                                        NodeStatus::Online => {
                                            let offline_node_addr = n_info.addr.clone();
                                            // println!("offline_node_addr: {}", offline_node_addr);

                                            let state_msg_vec = Message {
                                                id: helper::get_sys_time_in_nano(),
                                                sender: my_node_addr.clone(),
                                                kind: MessageKind::State,
                                                payload: StateType::ControlNodeStatus(
                                                    offline_node_addr.clone(),
                                                )
                                                .to_byte_array(),
                                            }
                                            .to_byte_array();
                                            // omergoksoy
                                            // burada online olan bir nodu'un offline olduğunu öğrendik
                                            // bu durumu diğer nodelara state olarak iletiyoruz
                                            for node_addr in node_list.to_node_list().iter() {
                                                if node_addr.eq(&offline_node_addr.clone()) == false
                                                    && node_addr.eq(&my_node_addr.clone()) == false
                                                {
                                                    // println!("node'u ilet => {}", helper::get_sys_time_in_nano() );
                                                    client_async(
                                                        node_addr.clone(),
                                                        state_msg_vec.clone(),
                                                    );
                                                }
                                            }
                                            update_node_status.push((
                                                offline_node_addr.clone(),
                                                NodeStatus::Offline,
                                            ));
                                            node_status_change.lock().unwrap().push((
                                                offline_node_addr.clone(),
                                                NodeStatus::Offline,
                                            ));
                                        }
                                        NodeStatus::Offline => {
                                            move_to_offline_node = n_index.clone();
                                        }
                                        NodeStatus::Unknown => {
                                            update_node_status
                                                .push((n_info.addr.clone(), NodeStatus::Offline));
                                            node_status_change
                                                .lock()
                                                .unwrap()
                                                .push((n_info.addr.clone(), NodeStatus::Unknown));
                                        }
                                    }
                                } else {
                                    if result.id != 9 {
                                        debug!("result: {:?}", result);
                                    }
                                }
                            }
                        }
                    }
                }

                if move_to_offline_node != usize::MAX {
                    node_list.lock().unwrap().remove(move_to_offline_node);
                    all_node_list_changed = true;
                } else {
                    for (n_addr, n_time) in update_time.iter() {
                        for n_info in node_list.lock().unwrap().iter_mut() {
                            if n_info.addr.eq(n_addr) {
                                n_info.last_access_time = n_time.clone();
                            }
                        }
                    }

                    for (o_node, n_status) in update_node_status.iter() {
                        for n_info in node_list.lock().unwrap().iter_mut() {
                            if n_info.addr.eq(o_node) {
                                n_info.status = n_status.clone();
                            }
                        }
                    }

                    // asenkron olduğu için aynı işlemi tekrar tekrar yapıyor
                    let mut update_sync_time = Vec::new();
                    let tmp_node_list = node_list.lock().unwrap().clone();
                    for n_info in tmp_node_list.iter() {
                        if my_node_addr.eq(&n_info.addr.clone()) == false {
                            if my_node_hash.lock().unwrap().eq(&n_info.node_hash) == false {
                                if n_info.status == NodeStatus::Online {
                                    let time_diff = helper::get_sys_time_in_millis()
                                        - n_info.synced_time_as_secs;
                                    if time_diff > 100 {
                                        update_sync_time.push(n_info.addr.clone());
                                        trace!(
                                            "Sync With  [ {} ]  => {}",
                                            time_diff,
                                            n_info.addr.clone()
                                        );

                                        node_list.send_state_to_all(
                                            my_node_addr.clone(),
                                            StateType::NodeList(node_list.to_node_list()),
                                        );
                                    }
                                }
                            }
                        }
                    }

                    node_list
                        .clone()
                        .set_sync_time(update_sync_time, helper::get_sys_time_in_millis());

                    if helper::control_nodes_hash(node_list.lock().unwrap().clone()) {
                        if all_node_list_changed == true {
                            all_node_list_changed = false;
                            let node_hash_cloned = my_node_hash.lock().unwrap().clone();
                            let current_list_hash = node_list_synced.lock().unwrap().clone();
                            if current_list_hash.eq(&node_hash_cloned.clone()) == false {
                                *node_list_synced.lock().unwrap() = node_hash_cloned.clone();
                                *node_hash_updated.lock().unwrap() = true;
                            }
                        }
                    } else {
                        all_node_list_changed = true;
                    }
                }

                if update_node_hash_value == true {
                    let new_node_list_hash = node_list.calculate_hash();
                    if current_node_hash
                        .lock()
                        .unwrap()
                        .clone()
                        .eq(&new_node_list_hash)
                        == false
                    {
                        all_node_list_changed = true;
                    }
                    *current_node_hash.lock().unwrap() = new_node_list_hash.clone();
                    for n_info in node_list.lock().unwrap().iter_mut() {
                        if my_node_addr.eq(&n_info.addr.clone()) {
                            if n_info.node_hash.eq(&new_node_list_hash.clone()) == false {
                                all_node_list_changed = true;
                                n_info.node_hash = new_node_list_hash.clone();
                            }
                        }
                    }
                }

                ping_time_diff = next_ping_time_diff;

                if ping_time_diff > 0 {
                    //thread::sleep(Duration::from_millis(50));
                }
            }
        });
    }

    fn thread_socket(&self) -> bool {
        let listener = TcpListener::bind(self.my_addr.clone());
        if listener.is_err() {
            return false;
        }
        let listener = listener.unwrap();
        let pool = ThreadPool::new(4);
        let msg_list_cloned = self.msg_list.clone();
        let my_addr = self.my_addr.clone();
        thread::spawn(move || loop {
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                let msg_list_cloned_inner = msg_list_cloned.clone();
                let my_addr_cloned = my_addr.clone();
                pool.execute(move || {
                    handle_connection::handle_connection(
                        my_addr_cloned,
                        stream,
                        msg_list_cloned_inner.clone(),
                    );
                });
            }
        });
        return true;
    }

    fn update_node_hash(&mut self) {
        let my_node_hash = self.node_list.calculate_hash();
        *self.node_hash.lock().unwrap() = my_node_hash.clone();
        for n_info in self.node_list.lock().unwrap().iter_mut() {
            if self.my_addr.eq(&n_info.addr.clone()) {
                n_info.node_hash = my_node_hash.clone();
            }
        }
    }

    pub fn add_node_to_lists(&mut self, node_list: Vec<String>) -> bool {
        let mut updated = false;
        for node_addr in node_list.iter() {
            if self.add_node_to_list(node_addr.to_string()) == true {
                updated = true;
            }
        }
        return updated;
    }

    pub fn add_node_to_list(&mut self, node_addr: String) -> bool {
        if node_addr.len() == 0 {
            return false;
        }
        let mut updated = false;
        let mut node_exist = false;
        for n_info in self.node_list.lock().unwrap().iter() {
            if node_addr.eq(&n_info.addr.clone()) {
                node_exist = true;
            }
        }
        if node_exist == false {
            updated = true;
            self.node_list.lock().unwrap().push(NodeDetails {
                addr: node_addr.clone(),
                status: if node_addr.eq(&self.my_addr) {
                    NodeStatus::Online
                } else {
                    NodeStatus::Unknown
                },
                synced_time_as_secs: 0,
                last_access_time: 0,
                node_hash: String::new(),
            });
            self.node_list.clone().set_sync_time(Vec::new(), 0);
        }
        updated
    }

    fn load_from_disk(&self) -> Vec<String> {
        if self.store_node_list_active == true {
            match fs::read_to_string(format!(
                "{}.json",
                self.my_addr.clone().replace(".", "_").replace(":", "_")
            )) {
                Ok(data) => {
                    let payload_result: serde_json::Result<Vec<String>> =
                        serde_json::from_str(&data);
                    match payload_result {
                        Ok(node_list) => node_list,
                        Err(_) => Vec::new(),
                    }
                }
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        }
    }
    pub fn store_node_list_active(&mut self, status: bool) {
        self.store_node_list_active = status;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventType {
    OnNodesSynced(String),
    OnNodeStatusChanged(String, NodeStatus),
    OnMessage(Message),
    OnWait(),
}
