use std::path::Path;
use crate::*;
use std::fs;
use smashnet::curl::Curler;
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

pub fn pong() -> Result<String, String> {
    Ok("pong from switch!".to_string())
}

pub fn read_file(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();
    let path = args[0].clone();
    let exists = Path::new(&path).exists();
    if !exists {
        return Err("requested file does not exist!".to_string());
    } else {
        return match fs::read_to_string(path) {
            Ok(data) => Ok(data),
            Err(e) => Err(format!("While reading file, {}", e))
        }
    }
}

pub fn download_file(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();
    let url = args[0].clone();
    let location = args[1].clone();
    
    let progress = |total: f64, current: f64| {
        context.send_progress(Progress::new(
        "Downloading".to_string(), 
        "downloading a file".to_string(), 
        current/total));
    };

    let result = Curler::new()
        .progress_callback(&progress)
        .download(url, location);

    return match result {
        Ok(()) => Ok("File downloaded successfully!".to_string()),
        Err(e) => Err(format!("Error during download, error name: {:?}", e))
    }
}

pub fn delete_file(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();

    let path = args[0].clone();
    let exists = Path::new(&path).exists();
    if !exists {
        return Err("requested file already does not exist.".to_string());
    } else {
        return match fs::remove_file(path) {
            Ok(_) => Ok("The file was removed successfully".to_string()),
            Err(e) => Err(format!("{}", e))
        }
    }
}

pub fn write_file(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();
    let path = args[0].clone();
    let exists = Path::new(&path).exists();
    if exists {
        // delete existing file, if present
        match fs::remove_file(path.clone()) {
            Ok(_) => println!("Deleted existing file successfully."),
            Err(e) => return Err(format!("Could not delete existing file! Reason: {:?}", e))
        }
    } 

    return match fs::write(path, args[1].clone()) {
        Ok(_) => Ok("The file was written successfully".to_string()),
        Err(e) => Err(format!("Could not write file. Reason: {:?}", e))
    }
}

pub fn get_md5(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();
    let path = args[0].clone();
    let exists = Path::new(&path).exists();
    if !exists {
        return Err("requested file does not exist!".to_string());
    } else {
        // read the file
        let data = match fs::read(path) {
            Ok(data) => data,
            Err(e) => {
                return Err(format!("while reading file, {:?}", e));
            }
        };
        // compute the md5 and return the value
        let digest = md5::compute(data);
        return Ok(format!("{:x}", digest));
    }
}

pub fn unzip(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();
    let filepath = args[0].clone();
    let destination = args[1].clone();

    if !Path::new(&filepath).exists() {
        return Err(format!("file {} does not exist!", filepath));
    }
    if !Path::new(&filepath).is_file() {
        return Err(format!("path {} is not a file!", filepath));
    }

    if !Path::new(&destination).exists() {
        return Err(format!("path {} does not exist!", destination));
    }
    if !Path::new(&destination).is_dir() {
        return Err(format!("path {} is not a directory!", destination));
    }

    let mut zip = match unzipper::get_zip_archive(&filepath) {
        Ok(zip) => zip,
        Err(_) => return Err("Could not parse zip file!".to_string())
    };

    let count = zip.len();

    for file_no in 0..count {
        let mut file = zip.by_index(file_no).unwrap();
        if !file.is_file() {
            continue;
        }

        context.send_progress(Progress::new(
            "Extracting".to_string(), 
            format!("{}", file.name()), 
            (file_no as f64)/(count as f64)));
        
        let path = Path::new(&destination).join(file.name());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent);
        }

        let mut file_data = vec![];
        file.read_to_end(&mut file_data).unwrap();
        std::fs::write(path, file_data).unwrap();
    }

    Ok("unzip succeeded".to_string())
}

pub fn mkdir(context: &mut MessageContext) -> Result<String, String> {
    let dir = &context.arguments.as_ref().unwrap()[0];
    return match std::fs::create_dir_all(dir) {
        Ok(ok) => Ok(format!("{:?}", ok)),
        Err(err) => Ok(format!("{:?}", err)),
    };
}

pub fn list_all_files(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();
    let path = args[0].clone();
    if !Path::new(&path).exists() {
        return Err(format!("path {} does not exist!", path));
    }
    if !Path::new(&path).is_dir() {
        return Err(format!("path {} is not a directory!", path));
    }

    let mut subtree = DirTree{name: path.clone(), files: Vec::new(), dirs: Vec::new()};
    readDirAll(path, &mut subtree);
    
    let json = match serde_json::to_string(&subtree) {
        Ok(val) => val,
        Err(e) => {
            return Err(format!("Could not serialize to json DirTree. Error: {}", e)); 
        }
    };
    //println!("replying to list_all_files with a string of size: {}", json.len());
    Ok(json)
}

pub fn file_exists(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();
    let path = args[0].clone();
    let exists = Path::new(&path).exists() && Path::new(&path).is_file();
    Ok(exists.to_string())
}

pub fn dir_exists(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();
    let path = args[0].clone();
    let exists = Path::new(&path).exists() && Path::new(&path).is_dir();
    Ok(exists.to_string())
}

pub fn list_dir(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();
    let path = args[0].clone();
    if !Path::new(&path).exists() {
        return Err(format!("path {} does not exist!", path));
    }
    if !Path::new(&path).is_dir() {
        return Err(format!("path {} is not a directory!", path));
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
        let path_entry = PathEntry{path: fullpath, kind: kind};
        vec.push(path_entry);
    }
    let path_list = PathList{list: vec};
    let json = match serde_json::to_string(&path_list) {
        Ok(val) => val,
        Err(e) => {
            return Err(format!("Could not serialize to json PathList. Error: {}", e)); 
        }
    };
    //println!("replying to list_dir with: {}", &json);
    Ok(json)
}

pub fn get_request(context: &mut MessageContext) -> Result<String, String> {
    let args = context.arguments.as_ref().unwrap();
    let url = args[0].clone();

    let progress = |total: f64, current: f64| {
        context.send_progress(Progress::new(
        "Performing GET".to_string(), 
        "doing GET request".to_string(), 
        current/total));
    };

    let result = Curler::new()
        .progress_callback(&progress)
        .get(url);

    //println!("got result from GET");

    return match result {
        Ok(body) => Ok(body),
        Err(e) => Err(format!("Error during get: {}", e))
    }
}

pub(crate) fn register_defaults(engine: &mut RequestEngine) {
    // handler for a basic backend ping
    engine.register("ping", Some(0), |_|{pong()});
    // handler for reading a file as a string
    engine.register("read_file", Some(1), |context| {
        read_file(context)
    });
    // handler for downloading a file to a location
    engine.register("download_file", Some(2), |context| {
        download_file(context)
    });
    // handler for deleting a file
    engine.register("delete_file", Some(1), |context| {
        delete_file(context)
    });
    engine.register("write_file", Some(2), |context| {
        write_file(context)
    });
    engine.register("get_md5", Some(1), |context| {
        get_md5(context)
    });
    engine.register("unzip", Some(2), |context| {
        unzip(context)
    });
    engine.register("file_exists", Some(1), |context| {
        file_exists(context)
    });
    engine.register("dir_exists", Some(1), |context| {
        dir_exists(context)
    });
    engine.register("list_all_files", Some(1), |context| {
        list_all_files(context)
    });
    engine.register("list_dir", Some(1), |context| {
        list_dir(context)
    });
    engine.register("get_request", Some(1), |context| {
        get_request(context)
    });
    engine.register("exit_session", None, |context| {
        context.shutdown();
        Ok("session should be closed, so this will never be sent".to_string())
    });
    engine.register("exit_application", None, |_context| {
        unsafe { skyline::nn::oe::ExitApplication();}
        // application is now closed, so we cannot return meaningfully.
    });
    engine.register("log", None, |context| {
        let args = context.arguments.as_ref().unwrap();
        println!("Frontend Log: {}", args[0]);
        Ok("ok".to_string())
    });
    engine.register("mkdir", Some(1), |context| {mkdir(context)});
}


