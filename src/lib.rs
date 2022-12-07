use skyline_web::{WebSession};
use std::{collections::HashMap};
use crate::message::*;
use serde::{Serialize, Deserialize};

mod response;
mod message;
pub mod default_handlers;
mod unzipper;

/// progress data
#[derive(Serialize, Deserialize)]
pub struct Progress {
    pub title: String,
    pub info: String,
    /// an u32 in the range 0-100
    pub progress: f64
}

impl Progress {
    pub fn new(title: String, info: String, progress: f64) -> Self {
        return Progress { title: title, info: info, progress: progress.max(0.0).min(1.0) }
    }
}


/// An engine for streamlining the handling of backend requests by `skyline-web` applications.
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
    /// Creates a new RequestEngine, taking ownership of the session in the process.
    pub fn new(session: WebSession) -> Self {
        return RequestEngine{is_exit: false, session: session, handlers: HashMap::new()};
    }

    /// Registers a handler for requests with the given name.
    /// 
    /// # Arguments
    /// * `request_name` - the name of the request to listen for
    /// * `arg_count` - an optional number of arguments to expect. If `None` is supplied, this
    ///     argument does nothing. If `Some` is supplied, then the engine will validate that the
    ///     inbound request has the required arguments present before calling the registered 
    ///     handler. If the argument count is incorrect, the handler will not be called and an
    ///     error will be returned to the frontend instead.
    /// * `handler` - this is a closure or function, which takes a `MessageContext` and must return 
    ///     `Result<String, String>`. The returned value (`Ok` or `Err`) is then sent to the frontend
    ///     as an `accept()` or `reject()` on the original `Promise`. Note that the returned string 
    ///     can be populated with JSON data. Such JSON can then be used in the frontend via `JSON.parse()`
    ///     to retreive complex structures.
    /// 
    /// Example:
    /// ```
    /// engine.register("my_call_name", Some(3), |context| {
    ///     let args = context.arguments.unwrap();
    ///     return Ok(format!("args: {}, {}, {}", args[0], args[1], args[2]));
    /// })
    /// ```
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
    /// Default calls:
    /// * `ping` 
    ///     - returns ok if the backend responded to the request
    /// * `read_file` 
    ///     - returns the file's contents as a string
    /// * `download_file` 
    ///     - downloads the given file to the given location
    /// * `delete_file` 
    ///     - deletes the given file
    /// * `write_file` 
    ///     - writes the given string to the given file location
    /// * `get_md5`
    ///     - returns the md5 checksum of the given file
    /// * `unzip`
    ///     - unzips the given file as to the given location
    /// * `file_exists`
    ///     - returns whether the given path exists and is a file
    /// * `dir_exists`
    ///     - returns whether the given path exists and is a directory
    /// * `list_all_files`
    ///     - returns a tree structure of the given directory, recursively
    /// * `list_dir`
    ///     - returns a list of the files and directories in the given path (non recursive)
    /// * `get_request`
    ///     - performs a GET request (using `smashnet`) and returns the body as a string
    /// * `exit_session`
    ///     - signals the engine to shutdown and the session to close, unblocking `start()`
    /// * `exit_application`
    ///     - closes the application entirely (you will return to the home menu)
    pub fn register_defaults(&mut self) -> &mut Self {
        default_handlers::register_defaults(self);
        return self;
    }

    /// Start the request engine. This will block and internally loop until `shutdown()` 
    /// has been called by a handler (such as with `exitSession()` in the 
    /// `DefaultMessenger`, or via `context.shutdown()` in a registered custom handler);
    pub fn start(&mut self) {
        while !self.is_exit {
            println!("listening");
            // block until we get a message from the frontend
            let msg = self.session.recv_max(0x200000);
            let message = match serde_json::from_str::<Message>(&msg) {
                Ok(message) => {
                    message
                },
                Err(e) => {
                    let str = match &msg.len() {
                        0..=300 => msg.to_string(),
                        _ => format!("{} <truncated for performance>", &msg[0..299])
                    };
                    println!("Failed to deserialize message: {}\nError: {:?}", str, e);
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