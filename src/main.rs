use crossbeam_channel::{unbounded, Receiver, Sender};
use pretty_bytes::converter::convert;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, future, path};

fn main() {
    let current_path = env::current_dir()
        .expect("")
        .to_str()
        .expect("")
        .to_string();
    let args: Vec<String> = env::args().collect();
    let target = args.get(1).unwrap_or(&current_path);
    let path = path::Path::new(target);
    println!("Searching in path: {}", target);

    // handle file
    if !path.exists() {
        eprintln!("Invalid path: {}", &target);
    } else if path.is_file() {
        todo!("Handling file");
    } else if path.is_dir() {
        let size: f64 = size_of(path) as f64;
        //let size: f64 = size_of_dir(path) as f64;
        //let size: f64 = size_of_dir2(&PathBuf::from_str(target).unwrap()) as f64;
        println!("Total size is {} bytes ({})", size, convert(size));
    } else {
        todo!("Unknown type");
    }
}

fn size_of(path: &path::Path) -> u64 {
    let size = Arc::from(Mutex::new(Box::from(0 as u64)));
    let mut consumers = Vec::new();
    {
        let (producer, rx) = unbounded();
        let producer = Box::new(producer);

        for idx in 1..10 {
            let producer = producer.clone();
            let rx = rx.clone();
            let size = size.clone();

            consumers.push(std::thread::spawn(move || -> () {
                let p = producer.as_ref().clone();
                receiver(idx, rx.clone(), &p, &size);
            }));
        }

        walk(path, &producer.as_ref().clone(), &size.clone().as_ref());
    }

    for c in consumers {
        c.join().unwrap();
    }

    *size.clone().lock().unwrap().as_ref()
}

fn receiver(_: i32, r: Receiver<PathBuf>, p: &Sender<PathBuf>, c: &Mutex<Box<u64>>) {
    while let Ok(path) = r.recv_timeout(Duration::from_millis(500)) {
        walk(&path, p, c);
    }
}

fn walk(path: &path::Path, p: &Sender<PathBuf>, c: &Mutex<Box<u64>>) {
    if !path.is_dir() {
        return;
    }

    for item in path.read_dir().expect("Read dir") {
        if let Ok(entry) = item {
            let path = entry.path();
            if path.is_file() {
                let size = path.metadata().unwrap().len();
                {
                    let mut sum = c.lock().unwrap();
                    let sum2 = sum.as_mut();
                    //println!("Inc {} + {} ({})", *sum2, size, path.to_str().unwrap());
                    *sum2 = *sum2 + size;
                }
            } else if path.is_dir() {
                p.send(path).expect("");
            }
        }
    }
}

#[allow(dead_code)]
fn size_of_dir(path: &path::Path) -> u64 {
    if !path.is_dir() {
        return 0;
    }

    let mut count = 0;
    for item in path.read_dir().expect("Read dir") {
        if let Ok(entry) = item {
            let path = entry.path();
            if path.is_file() {
                count += path.metadata().unwrap().len();
            } else if path.is_dir() {
                count += size_of_dir(&path);
            }
        }
    }
    count
}
