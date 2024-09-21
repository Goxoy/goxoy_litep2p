use crate::structs::{ConvertVecToStruct, Message, MessageKind};
use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{Arc, Mutex},
};

pub fn handle_connection(
    my_addr: String,
    mut stream: TcpStream,
    msg_list: Arc<Mutex<Vec<Message>>>,
) {
    let mut read_buf = [0u8; 4096];
    match stream.read(&mut read_buf) {
        Ok(n) => {
            if n > 0 {
                let income_data = read_buf[0..n].to_vec().to_message_struct();
                msg_list.lock().unwrap().push(income_data);
                _ = stream.write(
                    &Message {
                        id: 0,
                        sender: my_addr.clone(),
                        kind: MessageKind::Ok,
                        payload: Vec::new(),
                    }
                    .to_byte_array(),
                );
            }
        }
        Err(_err) => {
            _ = stream.write_all("ERR".as_bytes());
        }
    }
}
