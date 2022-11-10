use skyline_web::{Webpage, WebSession};
use std::collections::HashMap;
use crate::message::*;
use serde::{Serialize, Deserialize};
use std::fmt;
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
    pub callback:  Box<dyn Fn(&MessageContext) -> ()>
}

impl RequestEngine {
    fn shutdown(mut self) {
        self.is_exit = true;
        self.session.exit();
    }

    fn new(session: WebSession) -> Self {
        return RequestEngine{is_exit: false, session: session, handlers: HashMap::new()};
    }

    fn register<S: ToString>(&mut self, request_name: S, arg_count: Option<usize>, handler: impl Fn(&MessageContext)->() + 'static) -> &mut Self {
        let name = request_name.to_string();
        self.handlers.insert(name.to_string(), Handler { 
            call_name: name, 
            arg_count: arg_count, 
            callback: Box::new(handler)
        });
        return self;
    }

    /// Registers the "default" handlers for some common functionality. 
    /// This aligns with the `nx-request-api` NPM package's DefaultMessenger.
    fn register_defaults(&mut self) -> &mut Self {
        default_handlers::register_defaults(self);
        return self;
    }

    /// This is the bulk of the operation. This function
    /// loops and blocks until shutdown() has been called by a handler.
    fn start(&mut self) {
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
            let handler = self.handlers.get(&call_name);
            match handler {
                Some(handler) => {
                    println!("handling {}", call_name);
                    let ctx = MessageContext::build(message, &self.session);
                    // if an expected arg count was specified in the handler,
                    // we must ensure that this is reality. If not, respond with an error.
                    if handler.arg_count.is_some() {
                        let count = handler.arg_count.unwrap();
                        // if the number of args is wrong, error out
                        match &ctx.arguments {
                            Some(args) => {
                                if args.len() != count {
                                    let error = format!("Incorrect number of arguments were provided for {}", &call_name);
                                    ctx.error(error.as_ref());
                                    continue;
                                }
                            },
                            None => {
                                let error = format!("No arguments were provided for {}", &call_name);
                                ctx.error(error.as_ref());
                                continue;
                            }
                        }
                    }
                    (handler.callback)(&ctx);
                },
                None => println!("No handler was registered for {}", &message.call_name)
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