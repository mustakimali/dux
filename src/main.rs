use crossbeam_channel::{unbounded, Receiver, Sender};
use pretty_bytes::converter::convert;
use std::fmt::Display;
use std::path::{Path, PathBuf};
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

    if !path.exists() {
        eprintln!("Invalid path: {}", &target);
    } else if path.is_file() {
        let stat = Stats::from_file(&path);
        println!("Total size is {}", stat);
    } else if path.is_dir() {
        // Single threaded
        // let size: f64 = size_of_dir_single_threaded(path) as f64;
        // println!("Total size is {} bytes ({})", size, convert(size));

        // Multi threaded
        let threads = num_cpus::get();
        let stat = size_of_dir(path, threads);
        println!("Total size is {}", stat);
    } else {
        eprintln!("Unknown type {}", target);
    }
}

#[derive(Default, Clone)]
struct Stats {
    size: u64,
    count: i32,
}
impl Stats {
    fn from_file(p: &Path) -> Self {
        Self {
            size: p.metadata().unwrap().len(),
            count: 1,
        }
    }
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

fn size_of_dir(path: &path::Path, num_threads: usize) -> Stats {
    let stat = Arc::from(Mutex::new(Stats::default()));
    let mut consumers = Vec::new();
    {
        let (producer, rx) = unbounded();
        let producer = Box::new(producer);

        for idx in 1..num_threads {
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

    // wait for all receiver to finish
    for c in consumers {
        c.join().unwrap();
    }

    stat.clone().lock().unwrap().clone()
}

#[allow(unused_variables)]
fn receiver(
    idx: usize,
    receiver: Receiver<PathBuf>,
    sender: &Sender<PathBuf>,
    stat: &Mutex<Stats>,
) {
    while let Ok(path) = receiver.recv_timeout(Duration::from_millis(50)) {
        walk(&path, sender, stat);
    }

    #[cfg(debug_assertions)]
    println!("Thread#{} ended", idx);
}

fn walk(path: &path::Path, sender: &Sender<PathBuf>, stat: &Mutex<Stats>) {
    // Optimisation
    // if !path.is_dir() {
    //     return;
    // }

    for entry in path.read_dir().expect("Read dir").flatten() {
        let path = entry.path();
        if path.is_file() {
            let size = path.metadata().unwrap().len();
            {
                let mut sum = stat.lock().unwrap();
                //println!("Inc {} + {} ({})", *sum2, size, path.to_str().unwrap());
                sum.size += size;
                sum.count += 1;
            }
        } else if path.is_dir() {
            sender.send(path).unwrap();
        }
    }
}

#[allow(dead_code)]
fn size_of_dir_single_threaded(path: &path::Path) -> u64 {
    if !path.is_dir() {
        return 0;
    }

    let mut count = 0;
    for entry in path.read_dir().expect("Read dir").flatten() {
        let path = entry.path();
        if path.is_file() {
            count += path.metadata().unwrap().len();
        } else if path.is_dir() {
            count += size_of_dir_single_threaded(&path);
        }
    }
    count
}
