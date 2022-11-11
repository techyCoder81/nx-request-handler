use skyline_web::{Webpage, WebSession};
use std::{collections::HashMap, error::Error};
use crate::message::*;
use serde::{Serialize, Deserialize};
use std::fmt;
use std::fmt::Display;
use crate::{response::Progress};


mod response;
mod message;
mod default_handlers;
mod unzipper;

pub struct RequestEngine {
    is_exit: bool,
    session: WebSession,
    handlers: HashMap<String, Handler>
}

struct Handler {
    pub call_name: String,
    pub arg_count: Option<usize>,
    pub callback:  Box<dyn Fn(&mut MessageContext) -> Result<String, String>>
}

impl RequestEngine {
    pub fn new(session: WebSession) -> Self {
        return RequestEngine{is_exit: false, session: session, handlers: HashMap::new()};
    }

    pub fn register<S: ToString>(
        &mut self, request_name: S, 
        arg_count: Option<usize>, 
        handler: impl Fn(&mut MessageContext)-> Result<String, String> + 'static) -> &mut Self {
        let name = request_name.to_string();
        self.handlers.insert(name.clone(), Handler { 
            call_name: name, 
            arg_count: arg_count, 
            callback: Box::new(handler)
        });
        return self;
    }

    /// Registers the "default" handlers for some common functionality. 
    /// This aligns with the `nx-request-api` NPM package's DefaultMessenger.
    pub fn register_defaults(&mut self) -> &mut Self {
        default_handlers::register_defaults(self);
        return self;
    }

    /// This is the bulk of the operation. This function
    /// loops and blocks until shutdown() has been called by a handler.
    pub fn start(&mut self) {
        while !self.is_exit {
            println!("listening");
            // block until we get a message from the frontend
            let msg = self.session.recv();
            let message = match serde_json::from_str::<Message>(&msg) {
                Ok(message) => {
                    message
                },
                Err(_) => {
                    println!("Failed to deserialize message: {}", &msg);
                    continue;
                }
            };
            let call_name = message.call_name.clone();

            // try to handle the message
            match self.handlers.contains_key(&call_name) {
                true => {
                    println!("handling {}", call_name);
                    let mut ctx = MessageContext::build(message, &self.session);
                    // if an expected arg count was specified in the handler,
                    // we must ensure that this is reality. If not, respond with an error.
                    let handler = self.handlers.get(&call_name).unwrap();
                    if handler.arg_count.is_some() {
                        let count = handler.arg_count.unwrap();
                        // if the number of args is wrong, error out
                        match ctx.arguments {
                            Some(ref args) => {
                                if args.len() != count {
                                    let error = format!("Incorrect number of arguments were provided for {}", &call_name);
                                    ctx.return_error(error.as_ref());
                                    continue;
                                }
                            },
                            None => {
                                let error = format!("No arguments were provided for {}", &call_name);
                                ctx.return_error(error.as_ref());
                                continue;
                            }
                        }
                    }

                    // run the registered callback
                    let result = (handler.callback)(&mut ctx);

                    // if the callback signaled a shutdown, then 
                    // shutdown the engine and session
                    if ctx.is_shutdown() {
                        return;
                    } else {
                        match result {
                            Ok(res) => ctx.return_ok(&res),
                            Err(err) => ctx.return_error(&err)
                        }
                    }
                },
                false => println!("No handler was registered for {}", &message.call_name)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use skyline_web::WebSession;
    use crate::{RequestEngine, response::Progress};
    

    #[test]
    fn can_construct() {
        let session = unsafe{std::mem::transmute::<&mut WebSession, WebSession>(&mut *std::ptr::null_mut() as &mut WebSession)};
        RequestEngine::new(session)
            .register_defaults()
            .register(
                "test",
                None,
                |context| context.send_progress(Progress{title: "Progress".to_owned(), info: "progress!".to_owned(), progress: 50}));
    }
}