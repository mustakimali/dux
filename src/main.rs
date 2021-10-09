use crossbeam_channel::{unbounded, Receiver, Sender};
use pretty_bytes::converter::convert;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, path};

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

    fn add_file(&mut self, p: &Path) -> Result<(), std::io::Error> {
        let size = p.metadata()?.len();
        self.count += 1;
        self.size += size;

        Ok(())
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

impl std::ops::AddAssign for Stats {
    fn add_assign(&mut self, rhs: Self) {
        self.count += rhs.count;
        self.size += rhs.size;
    }
}

impl<'a> std::iter::Sum<&'a Stats> for Stats {
    fn sum<I: Iterator<Item = &'a Stats>>(iter: I) -> Self {
        let mut result = Self::default();
        for stat in iter {
            result.count += stat.count;
            result.size += stat.size;
        }
        result
    }
}

#[cfg(debug_assertions)]
impl Drop for Stats {
    fn drop(&mut self) {
        println!("Drop {}", self);
    }
}

fn main() {
    let current_path = env::current_dir()
        .expect("")
        .to_str()
        .expect("")
        .to_string();
    let args: Vec<String> = env::args().collect();
    let target = args.get(1).unwrap_or(&current_path);
    let path = path::Path::new(target);

    #[cfg(debug_assertions)]
    println!("Searching in path: {}", target);

    if !path.exists() {
        eprintln!("Invalid path: {}", &target);
    } else if path.is_file() {
        let stat = Stats::from_file(path);
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

fn size_of_dir(path: &path::Path, num_threads: usize) -> Stats {
    let mut stats = Vec::new();
    let mut consumers = Vec::new();
    {
        let (producer, rx) = unbounded();

        for idx in 1..num_threads {
            let producer = producer.clone();
            let rx = rx.clone();

            consumers.push(std::thread::spawn(move || {
                receiver(idx, rx, &producer)
            }));
        }

        // walk the root folder
        stats.push(walk(path, &producer.clone()));
    }

    // wait for all receiver to finish
    for c in consumers {
        let stat = c.join().unwrap();
        stats.push(stat);
    }

    stats.iter().sum()
}

#[allow(unused_variables)]
fn receiver(idx: usize, receiver: Receiver<PathBuf>, sender: &Sender<PathBuf>) -> Stats {
    let mut stat = Stats::default();
    while let Ok(path) = receiver.recv_timeout(Duration::from_millis(2)) {
        let newstat = walk(&path, sender);
        stat += newstat;
    }

    #[cfg(debug_assertions)]
    println!("Thread#{} ended", idx);

    stat
}

fn walk(path: &path::Path, sender: &Sender<PathBuf>) -> Stats {
    #[cfg(debug_assertions)]
    println!("{}", path.to_str().unwrap());

    let mut stat = Stats::default();

    // Optimisation
    // if !path.is_dir() {
    //     return;
    // }

    for entry in path.read_dir().expect("Read dir").flatten() {
        let path = entry.path();
        if path.is_file() {
            //println!("Inc {} + {} ({})", *sum2, size, path.to_str().unwrap());
            stat.add_file(&path).unwrap();
        } else if path.is_dir() {
            //sender.send(path).unwrap();
            sender.try_send(path).unwrap();
        }
    }
    stat
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
