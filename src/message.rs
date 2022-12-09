use serde::{Serialize, Deserialize};
use skyline_web::WebSession;
use std::fmt;
use crate::response::*;
use crate::Progress;
use serde_json::json;

/// this represents the message format that we will
/// receive from the frontend.
#[derive(Serialize, Deserialize)]
pub struct Message {
    /// the unique ID of this request interaction, used to ensure
    /// correct matching of request and associated response
    pub id: String,
    /// the name of this call
    pub call_name: String,
    /// the optional list of arguments
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
    /// the unique ID of this request interaction, used to ensure
    /// correct matching of request and associated response
    pub id: String,
    /// the name of this call
    pub call_name: String,
    /// the optional list of arguments
    pub arguments: Option<Vec<String>>,
    /// the websession (USE GREAT CARE IN OPERATING ON THIS.)
    pub session: &'a WebSession,
    /// whether we are signalling intent to shutdown the engine
    is_shutdown: bool
}

impl <'a>MessageContext<'a> {
    /// builds the `MessageContext` for a handler to consume.
    pub(crate) fn build(message: Message, session: &WebSession) -> MessageContext {
        return MessageContext { id: message.id, call_name: message.call_name, arguments: message.arguments, session: session, is_shutdown: false }
    }
    /// immediately closes the session, and then signals that the engine
    /// will shutdown and unblock the `start()` thread upon completion of
    /// the current handler's operations.
    pub fn shutdown(&mut self) {
        self.session.exit();
        self.session.wait_for_exit();
        self.is_shutdown = true;
    }
    /// whether the engine has been signalled to shut down
    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown
    }
    /// sends the given `Progress` struct to the frontend, for progress 
    /// reporting of long-running operations.
    pub fn send_progress(&self, progress: Progress) {
        self.session.send(&serde_json::to_string(&StringResponse{
            id: "progress".to_string(), 
            message: serde_json::to_string(&progress)
                .unwrap()
                .replace("\r", "").replace("\0", "").replace("\\", "\\\\").replace("\"", "\\\"").replace("\t", "    ").trim().to_string(), 
            more: false
        }).unwrap());
        //println!("sent progress: {}", progress.progress);
    }
    pub(crate) fn return_bool(&self, result: bool) {
        //println!("Sending {}", result);
        self.return_ok(result.to_string().as_str());
    }
    fn return_result(&self, orig_message: &str, is_ok: bool) {
        let cleaned_message = orig_message.replace("\r", "").replace("\0", "").replace("\\", "\\\\").replace("\"", "\\\"").replace("\t", "    ");
        let message = cleaned_message.trim();
        let total_length = message.len();
        let mut index = 0;

        // send the data in chunks
        while index < total_length {
            let mut end_index = (index + CHUNK_SIZE).min(total_length);
            let mut slice = &message[index..end_index];
            let chars = slice.chars();
            while slice.len() > 5 && (chars.nth_back(0).unwrap() == '\\' || chars.nth_back(0).unwrap() == '\\') {
                end_index = end_index + 1;
                slice = &message[index..end_index];
            }
            
            let data = serde_json::to_string(&OkOrErrorResponse{ 
                id: self.id.clone(), ok: is_ok, message: slice.to_string(), more: (end_index < total_length)
            }).unwrap();
            if data.len() < 500 {
                println!("Sending chunk:\n'{}'", data);
            } else {
                println!("Sending chunk of lenth: {}", data.len());
            }
            self.session.send(&data);
            index = end_index;
            //println!("Chunked send percentage: {}%", 100.0 * index as f32/total_length as f32)
        }
    }
    pub(crate) fn return_ok(&self, message: &str) {
        self.return_result(message, true);
    }
    pub(crate) fn return_error(&self, message: &str) {
        self.return_result(message, false);
    }
}
const CHUNK_SIZE: usize = 25000;