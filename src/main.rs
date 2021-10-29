use crossbeam_channel::{unbounded, Receiver, Sender};
use pretty_bytes::converter::convert as humanize_byte;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, path};

#[derive(Default)]
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
            humanize_byte(self.size as f64),
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
        let cores = num_cpus::get().to_string();
        let cores = std::env::var("WORKERS").unwrap_or(cores).parse().unwrap();
        let stat = size_of_dir(path, cores);

        println!("Total size is {}", stat);
    } else {
        eprintln!("Unknown type {}", target);
    }
}

fn size_of_dir(path: &path::Path, num_threads: usize) -> Stats {
    let mut stats = Vec::new();
    let largest_files = Arc::new(Mutex::new(
        priority_queue::PriorityQueue::<String, u64>::with_capacity(20),
    ));
    let mut consumers = Vec::new();
    {
        let (producer, rx) = unbounded();

        for idx in 0..num_threads {
            let producer = producer.clone();
            let largest_files = largest_files.clone();
            let rx = rx.clone();

            consumers.push(std::thread::spawn(move || {
                worker(idx, rx, &producer, &largest_files)
            }));
        }

        #[cfg(debug_assertions)]
        println!("Total {} worker spwaned", consumers.len());

        // walk the root folder
        stats.push(walk(path, &producer, &largest_files));

        #[cfg(debug_assertions)]
        println!("Total {} items in queue", producer.len());
    } // extra block so the channel is dropped early,
      // therefore all threads waiting for new message will encounter the
      // exit codition and will run to the end.

    // wait for all receiver to finish
    for c in consumers {
        let stat = c.join().unwrap();
        stats.push(stat);
    }

    println!("Largest files:");
    for (path, size) in largest_files
        .clone()
        .as_ref()
        .lock()
        .unwrap()
        .clone()
        .into_sorted_iter()
        .take(10)
    {
        println!("{}\t{}", humanize_byte(size as f64), path);
    }
    println!("");

    stats.iter().sum()
}

#[allow(unused_variables)]
fn worker(
    idx: usize,
    receiver: Receiver<PathBuf>,
    sender: &Sender<PathBuf>,
    f: &Arc<Mutex<priority_queue::PriorityQueue<String, u64>>>,
) -> Stats {
    let mut stat = Stats::default();
    while let Ok(path) = receiver.recv_timeout(Duration::from_millis(50)) {
        let newstat = walk(&path, sender, f);
        stat += newstat;

        #[cfg(debug_assertions)]
        println!("#{} - {}", idx, &path.to_str().unwrap());
    }

    #[cfg(debug_assertions)]
    println!("Thread#{} ended", idx);

    stat
}

fn walk(
    path: &path::Path,
    sender: &Sender<PathBuf>,
    f: &Arc<Mutex<priority_queue::PriorityQueue<String, u64>>>,
) -> Stats {
    let mut stat = Stats::default();

    // Optimisation (makes it faster)
    // if !path.is_dir() {
    //     return;
    // }
    if let Err(e) = path.read_dir() {
        eprintln!("Error {} ({})", e, path.to_str().unwrap());
        return stat;
    } else if let Ok(dir_items) = path.read_dir() {
        for entry in dir_items.flatten() {
            let path = entry.path();
            if path.is_file() {
                stat.add_file(&path).unwrap();
                let size = path.metadata().unwrap().len();
                f.as_ref()
                    .lock()
                    .unwrap()
                    .push(path.to_str().unwrap().into(), size);
            } else if path.is_dir() {
                // publish message to the channel
                sender.try_send(path).unwrap();
            }
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
