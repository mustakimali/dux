use clap::Parser;
use cli_table::{print_stdout, Style, Table};
use crossbeam_channel::{unbounded, Receiver, Sender};
use pretty_bytes::converter::convert as humanize_byte;
use priority_queue::PriorityQueue;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, path};

mod priority_queue;

#[derive(Parser, Debug, Clone)]
#[clap(name = "dux", version = clap::crate_version!(), version = clap::crate_version!(), author = clap::crate_authors!(), about = clap::crate_description!())]
struct CliArg {
    #[clap(short('l'), long, about("Lists top 10 largest files"))]
    list_large_files: bool,
    #[clap(about("The folder to use (default to current directory)"))]
    path: Option<String>,
}

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
    let cli_args = CliArg::parse();

    let target = cli_args.path.clone().unwrap_or_else(|| {
        env::current_dir()
            .expect("")
            .to_str()
            .expect("")
            .to_string()
    });
    let path = path::Path::new(&target);

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
        let stat = size_of_dir(path, cores, &cli_args);

        println!("Total size is {}", stat);
    } else {
        eprintln!("Unknown type {}", target);
    }
}

fn size_of_dir(path: &path::Path, num_threads: usize, args: &CliArg) -> Stats {
    let mut stats = Vec::new();
    let largest_files = Arc::new(Mutex::new(PriorityQueue::new(10)));
    let mut consumers = Vec::new();
    {
        let (producer, rx) = unbounded();

        for idx in 0..num_threads {
            let producer = producer.clone();
            let largest_files = largest_files.clone();
            let track_lage_files = args.list_large_files;
            let rx = rx.clone();

            consumers.push(std::thread::spawn(move || {
                worker(idx, rx, &producer, &largest_files, track_lage_files)
            }));
        }

        // walk the root folder
        stats.push(walk(path, &producer, &largest_files, args.list_large_files));
    } // extra block so the channel is dropped early,
      // therefore all threads waiting for new message will encounter the
      // exit codition and will run to the end.

    // wait for all receiver to finish
    for c in consumers {
        let stat = c.join().unwrap();
        stats.push(stat);
    }

    if args.list_large_files {
        println!("Largest files:");
        let wd_len = path.to_str().unwrap().len() + 1;
        let mut table_items = Vec::default();
        for (path, size) in largest_files.lock().unwrap().get() {
            table_items.push(vec![humanize_byte(size as f64), truncate(&path[wd_len..])]);
        }
        let table = table_items.table().title(vec!["Size", "File"]).bold(true);
        print_stdout(table).expect("failed to print largest files");
        println!();
    }

    stats.iter().sum()
}

fn truncate(path: &str) -> String {
    const MAX_WIDTH: usize = 80;
    if path.len() <= MAX_WIDTH {
        return path.to_string();
    }
    let mid = std::cmp::min(path.len() / 2, MAX_WIDTH / 2);

    let mut result = String::new();
    result.push_str(&path[..mid]);
    result.push_str("...");
    result.push_str(&path[path.len() - mid..]);

    result
}

#[allow(unused_variables)]
fn worker(
    idx: usize,
    receiver: Receiver<PathBuf>,
    sender: &Sender<PathBuf>,
    large_files: &Arc<Mutex<PriorityQueue>>,
    track_large_files: bool,
) -> Stats {
    let mut stat = Stats::default();
    while let Ok(path) = receiver.recv_timeout(Duration::from_millis(50)) {
        let newstat = walk(&path, sender, large_files, track_large_files);
        stat += newstat;
    }

    stat
}

fn walk(
    path: &path::Path,
    sender: &Sender<PathBuf>,
    large_files: &Arc<Mutex<PriorityQueue>>,
    track_large_files: bool,
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
                if track_large_files {
                    let size = path.metadata().unwrap().len();
                    large_files
                        .as_ref()
                        .lock()
                        .unwrap()
                        .push(path.to_str().unwrap().into(), size);
                }
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
