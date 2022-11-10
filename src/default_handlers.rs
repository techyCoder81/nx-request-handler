use std::path::Path;
use crate::*;
use std::fs;
use smashnet::types::*;
//use walkdir::*;
use std::io::Read;
use crate::response::{DirTree, PathEntry, PathList};

fn readDirAll(dir: String, tree: &mut DirTree) {
    //let tabs = "";
    //for (let i = 0; i < depth; ++i) {tabs += "\t";}
    let paths = fs::read_dir(dir).unwrap();
    for pathmaybe in paths {
        let path = pathmaybe.unwrap();
        let fullpath = path.path();
        let file_name = format!("{}", path.file_name().into_string().unwrap());
        if path.metadata().unwrap().is_file() {
            //println!("File: {}", file_name);
            tree.files.push(file_name);
        } else {
            //println!("Directory: {}", file_name);
            let mut subtree = DirTree{name: file_name, files: Vec::new(), dirs: Vec::new()};
            readDirAll(fullpath.into_os_string().into_string().unwrap(), &mut subtree);
            tree.dirs.push(subtree);
        }
    }
    
}

pub fn register_defaults(engine: &mut RequestEngine) {
    // handler for a basic backend ping
    engine.register("ping", Some(0), |context| {
        context.ok("pong from switch!");
    });
    // handler for reading a file as a string
    engine.register("read_file", Some(1), |context| {
        let args = context.arguments.as_ref().unwrap();
        let path = args[0].clone();
        let exists = Path::new(&path).exists();
        if !exists {
            context.error("requested file does not exist!");
        } else {
            match fs::read_to_string(path) {
                Ok(data) => context.ok(&data.replace("\r", "").replace("\0", "").replace("\\", "\\\\").replace("\"", "\\\"")),
                Err(e) => context.error(format!("{:?}", e).as_str())
            }
        }
    });
    // handler for downloading a file to a location
    engine.register("download_file", Some(2), |context| {
        let args = context.arguments.as_ref().unwrap();
        let url = args[0].clone();
        let location = args[1].clone();
        
        let result = Curler::new()
            //.progress_callback(|total, current| test(total, current))
            .download(url, location);

        match result {
            Ok(()) => context.ok("File downloaded successfully!"),
            Err(e) => context.error(format!("Error during download, error code: {}", e).as_str())
        }
    });
    // handler for deleing a file
    engine.register("delete_file", Some(1), |context| {
        let args = context.arguments.as_ref().unwrap();

        let path = args[0].clone();
        let exists = Path::new(&path).exists();
        if !exists {
            context.error("requested file already does not exist.");
        } else {
            match fs::remove_file(path) {
                Ok(version) => context.ok("The file was removed successfully"),
                Err(e) => context.error(format!("{:?}", e).as_str())
            }
        }
    });
    engine.register("write_file", Some(2), |context| {
        let args = context.arguments.as_ref().unwrap();
        let path = args[0].clone();
        let exists = Path::new(&path).exists();
        if exists {
            // delete existing file, if present
            match fs::remove_file(path.clone()) {
                Ok(version) => println!("Deleted existing file successfully."),
                Err(e) => context.error(format!("Could not delete existing file! Reason: {:?}", e).as_str())
            }
        } 

        match fs::write(path, args[1].clone()) {
            Ok(version) => context.ok("The file was written successfully"),
            Err(e) => context.error(format!("Could not write file. Reason: {:?}", e).as_str())
        }
    });
    engine.register("get_md5", Some(1), |context| {
        let args = context.arguments.as_ref().unwrap();
        let path = args[0].clone();
        let exists = Path::new(&path).exists();
        if !exists {
            context.error("requested file does not exist!");
        } else {
            // read the file
            let data = match fs::read(path) {
                Ok(data) => data,
                Err(e) => {
                    context.error(format!("while reading file, {:?}", e).as_str()); 
                    return;
                }
            };
            // compute the md5 and return the value
            let digest = md5::compute(data);
            context.ok(format!("{:x}", digest).as_str());
        }
    });
    engine.register("unzip", Some(2), |context| {
        let args = context.arguments.as_ref().unwrap();
        let filepath = args[0].clone();
        let destination = args[1].clone();

        if !Path::new(&filepath).exists() {
            context.error(format!("file {} does not exist!", filepath).as_str());
            return;
        }
        if !Path::new(&filepath).is_file() {
            context.error(format!("path {} is not a file!", filepath).as_str());
            return;
        }

        if !Path::new(&destination).exists() {
            context.error(format!("path {} does not exist!", destination).as_str());
            return;
        }
        if !Path::new(&destination).is_dir() {
            context.error(format!("path {} is not a directory!", destination).as_str());
            return;
        }

        let mut zip = match unzipper::get_zip_archive(&filepath) {
            Ok(zip) => zip,
            Err(_) => {
                context.error("Could not parse zip file!");
                return;
            }
        };
    
        let count = zip.len();
    
        for file_no in 0..count {
            let mut file = zip.by_index(file_no).unwrap();
            if !file.is_file() {
                continue;
            }
    
            //println!("progress: {}", file_no as f32/count as f32);
    
            let path = Path::new(&destination).join(file.name());
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent);
            }
    
            let mut file_data = vec![];
            file.read_to_end(&mut file_data).unwrap();
            std::fs::write(path, file_data).unwrap();
        }

        context.ok("unzip succeeded");
    });
    engine.register("file_exists", Some(1), |context| {
        let args = context.arguments.as_ref().unwrap();
        let path = args[0].clone();
        let exists = Path::new(&path).exists() && Path::new(&path).is_file();
        context.ok_bool(exists);
    });
    engine.register("dir_exists", Some(1), |context| {
        let args = context.arguments.as_ref().unwrap();
        let path = args[0].clone();
        let exists = Path::new(&path).exists() && Path::new(&path).is_dir();
        context.ok_bool(exists);
    });
    engine.register("list_all_files", Some(1), |context| {
        let args = context.arguments.as_ref().unwrap();
        let path = args[0].clone();
        if !Path::new(&path).exists() {
            context.error(format!("path {} does not exist!", path).as_str());
            return;
        }
        if !Path::new(&path).is_dir() {
            context.error(format!("path {} is not a directory!", path).as_str());
            return;
        }

        let mut subtree = DirTree{name: path.clone(), files: Vec::new(), dirs: Vec::new()};
        readDirAll(path, &mut subtree);
        
        let json = match serde_json::to_string(&subtree) {
            Ok(val) => val.replace("\r", "").replace("\\", "\\\\").replace("\"", "\\\""),
            Err(e) => {
                context.error(format!("Could not serialize to json DirTree. Error: {}", e).as_str()); 
                return;
            }
        };
        //println!("replying to list_all_files with a string of size: {}", json.len());
        context.ok(&json);
    });
    engine.register("list_dir", Some(1), |context| {
        let args = context.arguments.as_ref().unwrap();
        let path = args[0].clone();
        if !Path::new(&path).exists() {
            context.error(format!("path {} does not exist!", path).as_str());
            return;
        }
        if !Path::new(&path).is_dir() {
            context.error(format!("path {} is not a directory!", path).as_str());
            return;
        }

        let paths = fs::read_dir(path).unwrap();
        //println!("Paths...");
        let mut vec = Vec::new();
        for entry in paths {
            let fullpath = entry.unwrap().path().display().to_string();
            //println!("Path: {}", fullpath);
            let md = fs::metadata(fullpath.clone()).unwrap();
            let kind = match md.is_file() {
                true => 0,
                false => 1
            };
            let mut path_entry = PathEntry{path: fullpath, kind: kind};
            vec.push(path_entry);
        }
        let path_list = PathList{list: vec};
        let json = match serde_json::to_string(&path_list) {
            Ok(val) => val.replace("\r", "").replace("\\", "\\\\").replace("\"", "\\\""),
            Err(e) => {
                context.error(format!("Could not serialize to json PathList. Error: {}", e).as_str()); 
                return;
            }
        };
        //println!("replying to list_dir with: {}", &json);
        context.ok(&json);
    });
    engine.register("get_request", Some(1), |context| {
        let args = context.arguments.as_ref().unwrap();
        let url = args[0].clone();

        let result = Curler::new()
            //.progress_callback(|total, current| session.progress(current/total))
            .get(url);

        //println!("got result from GET");

        match result {
            Ok(body) => {
                if body.len() < 1000 {
                    //println!("Result: {}", body);
                } else {
                    //println!("body is very large, not println-ing.");
                }
                context.ok(&body.replace("\r", "").replace("\\", "\\\\").replace("\"", "\\\""));
            },
            Err(e) => {
                //println!("Error: {}", e);
                context.error(format!("Error during download: {}", e).as_str());
            }
        }
    });
}


