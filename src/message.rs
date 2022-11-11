use serde::{Serialize, Deserialize};
use skyline_web::WebSession;
use std::fmt;
use crate::response::*;

/// this represents the message format that we will
/// receive from the frontend.
#[derive(Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub call_name: String,
    pub arguments: Option<Vec<String>>
}


impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(id: {}, call_name: {})", self.id, self.call_name)
    }
}

/// this represents the message format that we hand
/// to user-defined handlers
pub struct MessageContext<'a> {
    pub id: String,
    pub call_name: String,
    pub arguments: Option<Vec<String>>,
    pub session: &'a WebSession,
    is_shutdown: bool
}

impl <'a>MessageContext<'a> {
    fn build(message: Message, session: &WebSession) -> MessageContext {
        return MessageContext { id: message.id, call_name: message.call_name, arguments: message.arguments, session: session, is_shutdown: false }
    }
    pub fn shutdown(&mut self) {
        self.session.exit();
        self.session.wait_for_exit();
        self.is_shutdown = true;
    }
    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown
    }
    pub fn send_progress(&self, progress: Progress) {
        self.session.send(&serde_json::to_string(&StringResponse{
            id: "progress".to_string(), 
            message: serde_json::to_string(&progress)
                .unwrap()
                .replace("\\", "\\\\")
                .replace("\"", "\\\"").trim().to_string(), 
            more: false
        }).unwrap());
    }
    fn return_bool(&self, result: bool) {
        //println!("Sending {}", result);
        self.return_ok(result.to_string().as_str());
    }
    fn return_ok(&self, message: &str) {
        //let message = message.replace("\r", "").replace("\\", "\\\\").replace("\"", "\\\"");
        //println!("Sending OK");

        let total_length = message.len();
        let mut index = 0;

        while index < total_length {
            let mut end_index = (index + CHUNK_SIZE).min(total_length);
            let mut slice = &message[index..end_index];
            while slice.chars().last().unwrap() == '\\' {
                end_index = end_index + 1;
                slice = &message[index..end_index];
            }
            
            let data = serde_json::to_string(&OkOrErrorResponse{ 
                id: self.id.clone(), ok: true, message: slice.trim().to_string(), more: (end_index < total_length)
            }).unwrap();
            //println!("Sending chunk:\n{}", data);
            self.session.send(&data);
            index = end_index;
            //println!("Chunked send percentage: {}%", 100.0 * index as f32/total_length as f32)
        }
    }
    fn return_error(&self, message: &str) {
        //let message = message.replace("\r", "").replace("\\", "\\\\").replace("\"", "\\\"");
        //println!("Sending ERROR");
        let total_length = message.len();
        let mut index = 0;
        while index < total_length {
            let mut end_index = (index + CHUNK_SIZE).min(total_length);
            let mut slice = &message[index..end_index];
            while slice.chars().last().unwrap() == '\\' {
                end_index = end_index + 1;
                slice = &message[index..end_index];
            }
            self.session.send(&serde_json::to_string(&OkOrErrorResponse{ 
                id: self.id.clone(), ok: false, message: slice.trim().to_string(), more: (end_index < total_length)
            }).unwrap());
            index = end_index;
            //println!("Chunked send percentage: {}%", 100.0 * index as f32/total_length as f32)
        }
    }
}
const CHUNK_SIZE: usize = 25000;