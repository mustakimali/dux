use crossbeam_channel::{unbounded, Receiver, Sender};
use pretty_bytes::converter::convert;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, path};

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
        let stat = size_of(path);
        //let size: f64 = size_of_dir(path) as f64;
        //let size: f64 = size_of_dir2(&PathBuf::from_str(target).unwrap()) as f64;
        println!("Total size is {}", stat);
    } else {
        todo!("Unknown type");
    }
}

#[derive(Default, Clone)]
struct Stats {
    size: u64,
    count: i32,
}
impl Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} bytes ({}) across {} items",
            self.size,
            convert(self.size as f64),
            self.count
        )
    }
}

fn size_of(path: &path::Path) -> Stats {
    let stat = Arc::from(Mutex::new(Stats::default()));
    let mut consumers = Vec::new();
    {
        let (producer, rx) = unbounded();
        let producer = Box::new(producer);

        for idx in 1..5 {
            let producer = producer.clone();
            let rx = rx.clone();
            let size = stat.clone();

            consumers.push(std::thread::spawn(move || -> () {
                let p = producer.as_ref().clone();
                receiver(idx, rx.clone(), &p, &size);
            }));
        }

        walk(path, &producer.as_ref().clone(), &stat.clone().as_ref());
    }

    for c in consumers {
        c.join().unwrap();
    }

    stat.clone().lock().unwrap().clone()
}

#[allow(unused_variables)]
fn receiver(idx: i32, r: Receiver<PathBuf>, p: &Sender<PathBuf>, c: &Mutex<Stats>) {
    while let Ok(path) = r.recv_timeout(Duration::from_millis(50)) {
        walk(&path, p, c);
    }

    #[cfg(debug_assertions)]
    println!("Thread#{} ended", idx);
}

fn walk(path: &path::Path, p: &Sender<PathBuf>, c: &Mutex<Stats>) {
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
                    //println!("Inc {} + {} ({})", *sum2, size, path.to_str().unwrap());
                    sum.size += size;
                    sum.count += 1;
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
