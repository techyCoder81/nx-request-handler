# nx-request-handler
A messaging handler for `skyline-web` plugins, as a streamlined backend implementation for the [nx-request-api NPM package](https://www.npmjs.com/package/nx-request-api).

## The Problem
This crate exists to solve a specific problem. In `skyline-web` applications, it can be hard to manage an appropriate separation of responsibilities between the rust backend and the javascript frontend. This is made worse by the lack of a comprehensive way to perform backend operations on the plugin with standard async patterns in the frontend, using promises to do work. Instead, there is only a global event listener which we can attach listeners to. Additionally, its a huge pain to write the backend itself. Most of these resolve to a big match statement, and typically people have not been able to return complex datatypes to the frontend. For these reasons, skyline-web applications tend to become unmaintainable in certain ways.

## The Solution
Fundamentally, this crate attempts to ensure that your backend implementation **feels like idiomatic rust**, and your frontend **feels like idiomatic typescript/javascript**. This crate and the [associated NPM package](https://www.npmjs.com/package/nx-request-api) provides a streamlined way to manage frontend-backend request operations in `skyline-web` applications. 

On the frontend, they `Promise`-ify all calls on the frontend side, provide a way for backend operations to report progress, and allow backend operations to return relatively complex data structures back to the frontend (see `list_dir_all`, which builds and then returns a recursive tree structure for a given directory). 

On the rust side, it uses a relatively straightforward builder pattern to register handlers for named backend operations and control the lifecycle the session. Individual, simple, and minimal backend calls can be defined, which in aggregate allow the frontend to perform complex sequences of operations. Additionally, the default included handlers provide most of the basic operations a plugin will likely need from the start.

When taken together, the system allows you to more easily centralize your business logic in the frontend, and treat the backend (plugin) more like a server, with discrete operations available for all of your basic operations, many of which are built-in by default.

# Example Usage

## Basic Usage
First, you must open a `WebSession`. 

Then, it's as simple as creating a `RequestEngine` using that session:
```rust
let engine = RequestEngine::new(my_session);
```

Then, register the default handlers if desired (see below for available default calls):
```rust
engine.register_defaults();
```
This will register the default implementations for various common needs, such as `read_file`, `write_file`, `get_md5`, `list_dir`, `list_dir_all`, `get_request`, `delete_file`, etc.


Register any custom callback handlers you may need:
```rust
engine.register("my_call_name", Some(3), |context| {
    let args = context.arguments.unwrap();
    return Ok(format!("args: {}, {}, {}", args[0], args[1], args[2]));
})
```
1. `my_call_name`: this is the string name of the operation to be registered.
2. `Some(3)`: this is the number of arguments we should expect. If the arguments present in the request from the frontend do not match this number, then the handler will not even be called, and instead an error will be returned to the frontend (the calling `Promise` will be rejected). If `None` is supplied instead, args will not be validated.
3. `|context| {...}`: this is a closure or function, which takes a `MessageContext` and must return `Result<String, String>`. The returned value (`Ok` or `Err`) is then sent to the frontend as an `accept()` or `reject()` on the original `Promise`. Note that the returned string can be populated with JSON data. Such JSON can then be used in the frontend via `JSON.parse()` to retreive complex structures. For example, one of the default handlers is `list_dir_all`, which returns recursively the entire directory structure starting at the given location, as a tree object.

Finally, just call `engine.start();`. This will block the current thread, listening for requests and delegating the calls to the appropriate registered handlers, automatically rejecting calls which do not have a registered handler or which do not have the appropriate arguments. To shutdown the engine, you can simply call `exitSession()` in the frontend api. Alternatively, you may call `context.shutdown()` arbitrarily in any registered handler. After the handler which called `shutdown()` returns, the engine will exit, and `start()` will return.

## Putting it all together:
Plugin side:\
(more in-depth example usage can be found in the [HDR Launcher backend](https://github.com/techyCoder81/hdr-launcher-react/blob/main/switch/src/lib.rs))
```rust
// Create a WebSession instance, using skyline-web
let session = Webpage::new()
    .htdocs_dir("hdr-launcher")
    .file("index.html", &HTML_TEXT)
    .file("index.js", &JS_TEXT)
    .file("logo_full.png", &LOGO_PNG)
    .background(skyline_web::Background::Default)
    .boot_display(skyline_web::BootDisplay::Black)
    .open_session(skyline_web::Visibility::InitiallyHidden).unwrap();

// show the session
session.show();

// create a RequestEngine, provided by nx-request-handler, to handle all requests
RequestEngine::new(session)
    .register_defaults()
    .register("get_sdcard_root", None, |context| {
        Ok("sd:/".to_string())
    })
    .register("is_installed", None, |context| {
        let exists = Path::new("sd:/ultimate/mods/hdr").exists();
        Ok(exists.to_string())
    })
    .register("call_with_args", Some(2), |context| {
        let args = context.arguments.unwrap();
        // report progress on long operation
        context.send_progress(Progress::new("Starting".to_string(), "getting started!".to_string(), 0));
        do_something_long(args[0]);

        // report more progress partway through
        context.send_progress(Progress::new("Halfway there".to_string(), "we are halfway there!".to_string(), 50));
        do_other_long_thing(args[1]);
        result
    })
    .register("get_version", None, |context| {
        let path = "sd:/ultimate/mods/hdr/ui/hdr_version.txt";
        let exists = Path::new(path).exists();
        if !exists {
            return Err("Version file does not exist!".to_string());
        } else {
            return match fs::read_to_string(path) {
                Ok(version) => Ok(version.trim().to_string()),
                Err(e) => Err(e.to_string())
            }
        }
    })
    .start();
```

Frontend for this example:\
(more in-depth example usage can be found in the [HDR Launcher frontend](https://github.com/techyCoder81/hdr-launcher-react/tree/main/src))
```typescript
import { Progress, DefaultMessenger } from "nx-request-api"

let messenger = new DefaultMessenger();
try {
    // examples using default messenger and register_defaults()
    // download a file to a location on sd, while providing a progress callback for display
    let download_result = await messenger.downloadFile(
        "https://url.com/hugefile.json", 
        "sd:/hugefile.json", 
        (p: Progress) => console.info("Operation: " + p.title + ", Progress: " + p.progress)
    );

    // read the contents of the file (in this case a json file),
    // and then parse the data into an object.
    let contents = await messenger.readFile("sd:/hugefile.json");
    let obj = JSON.parse(contents);
    console.info(obj.some_field);


    // generic invocation examples for custom handlers. 
    // These examples align with the custom handlers registered in the above Rust example.
    // simple string-based request, no arguments
    let version = await messenger.customRequest("get_sdcard_root", null);

    // string-based request, with three arguments
    let result = await messenger.customRequest("call_with_args", ["arg1", "arg2", "arg3"]);

    // request which returns a bool instead of a string
    let is_installed = await messenger.booleanRequest("is_installed", null);

    // another example of a default message call
    messenger.exitSession();
} catch (e) { 
    // This will be called if any of the requests are rejected. 
    // You can also use .then() and .catch() on the individual calls.
    console.error(e); 
}
```
Note: it is also possible to `extend` the `DefaultMessenger` or the `BasicMessenger` classes to abstract away some of the work of custom calls.

# Default calls
When using `DefaultMessenger` in the frontend, and calling `register_defaults()` on the backend `RequestEngine`,  the following operations will be supported by default:
* `ping` 
    - returns ok if the backend responded to the request
* `read_file` 
    - returns the file's contents as a string
* `download_file` 
    - downloads the given file to the given location
* `delete_file` 
    - deletes the given file
* `write_file` 
    - writes the given string to the given file location
* `get_md5`
    - returns the md5 checksum of the given file
* `unzip`
    - unzips the given file as to the given location
* `file_exists`
    - returns whether the given path exists and is a file
* `dir_exists`
    - returns whether the given path exists and is a directory
* `list_all_files`
    - returns a tree structure of the given directory, recursively
* `list_dir`
    - returns a list of the files and directories in the given path (non recursive)
* `get_request`
    - performs a GET request (using `smashnet`) and returns the body as a string
* `exit_session`
    - signals the engine to shutdown and the session to close, unblocking `start()`
* `exit_application`
    - closes the application entirely (you will return to the home menu)