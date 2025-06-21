use clap::Parser;
use cli_table::{print_stdout, Style, Table};
use crossbeam_channel::{unbounded, Receiver, Sender};
use pretty_bytes::converter::convert as humanize_byte;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fmt::Display;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, io};
use thousands::Separable;

type LargeFilesHeap = BinaryHeap<Reverse<(u64, String)>>;

#[derive(Parser, Debug, Clone)]
#[clap(name = "dux")]
struct CliArg {
    /// Lists top 10 largest files
    #[clap(short('l'), long)]
    list_large_files: bool,

    /// Group files by extension
    #[clap(short('g'), long)]
    group_extensions: bool,

    /// The folder to use (default to current directory)
    #[clap(name = "path")]
    path: Option<String>,
}

#[derive(Default)]
struct Stats {
    size: u64,
    count: i32,
    ext: HashMap<String, (u64, u64)>, // (size, count)
    track_ext: bool,
}

impl Stats {
    fn new(track_ext: bool) -> Self {
        Self {
            track_ext,
            ..Default::default()
        }
    }

    fn from_file(path: &Path, track_ext: bool) -> io::Result<Self> {
        let metadata = path.metadata()?;
        let mut stats = Self {
            size: metadata.len(),
            count: 1,
            track_ext,
            ..Default::default()
        };
        stats.add_extension(&metadata, path);
        Ok(stats)
    }

    fn add_file(&mut self, path: &Path) -> io::Result<Metadata> {
        let metadata = path.metadata()?;
        self.size += metadata.len();
        self.count += 1;
        self.add_extension(&metadata, path);
        Ok(metadata)
    }

    fn add_extension(&mut self, metadata: &Metadata, path: &Path) {
        if !self.track_ext || !metadata.is_file() {
            return;
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let (size, count) = self.ext.get(ext).copied().unwrap_or_default();
            self.ext
                .insert(ext.to_string(), (size + metadata.len(), count + 1));
        }
    }
}

impl Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.track_ext && !self.ext.is_empty() {
            let mut sorted_ext: Vec<_> = self.ext.iter().collect();
            sorted_ext.sort_by(|a, b| b.1.cmp(a.1));

            let table_items: Vec<_> = sorted_ext
                .into_iter()
                .map(|(ext, (size, count))| {
                    vec![ext.clone(), count.to_string(), humanize_byte(*size as f64)]
                })
                .collect();

            let table = table_items
                .table()
                .title(vec!["Extension", "#", "Size"])
                .bold(true);
            print_stdout(table).expect("failed to print table");
            writeln!(f)?;
        }

        write!(
            f,
            "Total size is {} bytes ({}) across {} items",
            self.size.separate_with_commas(),
            humanize_byte(self.size as f64),
            self.count.separate_with_commas()
        )
    }
}

impl std::ops::AddAssign for Stats {
    fn add_assign(&mut self, rhs: Self) {
        self.count += rhs.count;
        self.size += rhs.size;

        for (ext, (size, count)) in rhs.ext {
            let (existing_size, existing_count) = self.ext.get(&ext).copied().unwrap_or_default();
            self.ext
                .insert(ext, (existing_size + size, existing_count + count));
        }
    }
}

impl<'a> std::iter::Sum<&'a Stats> for Stats {
    fn sum<I: Iterator<Item = &'a Stats>>(iter: I) -> Self {
        iter.fold(Stats::default(), |mut acc, stat| {
            acc += stat.clone();
            acc
        })
    }
}

impl Clone for Stats {
    fn clone(&self) -> Self {
        Self {
            size: self.size,
            count: self.count,
            ext: self.ext.clone(),
            track_ext: self.track_ext,
        }
    }
}

fn main() {
    let cli_args = CliArg::parse();
    let target = get_target_path(&cli_args);

    #[cfg(debug_assertions)]
    println!("Searching in path: {}", target);

    let path = Path::new(&target);
    match classify_path(path) {
        PathType::NotFound => eprintln!("Invalid path: {}", target),
        PathType::File => match Stats::from_file(path, cli_args.group_extensions) {
            Ok(stats) => println!("{}", stats),
            Err(e) => eprintln!("Error reading file: {}", e),
        },
        PathType::Directory => {
            let cores = get_worker_count();
            let stats = size_of_dir(path, cores, &cli_args);
            println!("{}", stats);
        }
        PathType::Unknown => eprintln!("Unknown type {}", target),
    }
}

enum PathType {
    NotFound,
    File,
    Directory,
    Unknown,
}

fn classify_path(path: &Path) -> PathType {
    if !path.exists() {
        PathType::NotFound
    } else if path.is_file() {
        PathType::File
    } else if path.is_dir() {
        PathType::Directory
    } else {
        PathType::Unknown
    }
}

fn get_target_path(cli_args: &CliArg) -> String {
    cli_args.path.clone().unwrap_or_else(|| {
        env::current_dir()
            .expect("Failed to get current directory")
            .to_string_lossy()
            .to_string()
    })
}

fn get_worker_count() -> usize {
    env::var("WORKERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(num_cpus::get)
}

fn size_of_dir(path: &Path, num_threads: usize, args: &CliArg) -> Stats {
    let (sender, receiver) = unbounded();

    let consumers: Vec<_> = (0..num_threads)
        .map(|idx| {
            let sender = sender.clone();
            let receiver = receiver.clone();
            let args = args.clone();

            std::thread::spawn(move || worker(idx, receiver, &sender, &args))
        })
        .collect();

    let (root_stats, mut root_files) = walk(path, &sender, args);
    drop(sender); // Signal threads to exit

    let mut all_stats = vec![root_stats];
    for consumer in consumers {
        let (stats, files) = consumer.join().expect("Worker thread panicked");
        all_stats.push(stats);
        merge_largest_files(&mut root_files, files);
    }

    if args.list_large_files {
        print_largest_files(&root_files, path);
    }

    Stats {
        track_ext: args.group_extensions,
        ..all_stats.iter().sum()
    }
}

fn print_largest_files(largest_files: &LargeFilesHeap, root_path: &Path) {
    println!("Largest files:");
    let wd_len = root_path.to_string_lossy().len() + 1;

    let files: Vec<_> = largest_files.clone().into_sorted_vec();
    let table_items: Vec<_> = files
        .into_iter()
        .map(|Reverse((size, path))| vec![humanize_byte(size as f64), truncate(&path[wd_len..])])
        .collect();

    let table = table_items.table().title(vec!["Size", "File"]).bold(true);
    print_stdout(table).expect("Failed to print largest files");
    println!();
}

fn truncate(path: &str) -> String {
    const MAX_WIDTH: usize = 80;
    if path.len() <= MAX_WIDTH {
        return path.to_string();
    }

    let mid = std::cmp::min(path.len() / 2, MAX_WIDTH / 2);
    format!("{}...{}", &path[..mid], &path[path.len() - mid..])
}

fn worker(
    _idx: usize,
    receiver: Receiver<PathBuf>,
    sender: &Sender<PathBuf>,
    args: &CliArg,
) -> (Stats, LargeFilesHeap) {
    let mut stats = Stats::new(args.group_extensions);
    let mut large_files = BinaryHeap::new();

    while let Ok(path) = receiver.recv_timeout(Duration::from_millis(50)) {
        let (path_stats, path_files) = walk(&path, sender, args);
        stats += path_stats;
        merge_largest_files(&mut large_files, path_files);
    }

    (stats, large_files)
}

fn walk(path: &Path, sender: &Sender<PathBuf>, args: &CliArg) -> (Stats, LargeFilesHeap) {
    let mut stats = Stats::new(args.group_extensions);
    let mut large_files = BinaryHeap::new();

    let entries = match path.read_dir() {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading {}: {}", path.display(), e);
            return (stats, large_files);
        }
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();

        if entry_path.is_file() {
            if let Ok(metadata) = stats.add_file(&entry_path) {
                if args.list_large_files {
                    track_large_file(&mut large_files, &entry_path, metadata.len());
                }
            }
        } else if entry_path.is_dir() {
            let _ = sender.try_send(entry_path);
        }
    }

    (stats, large_files)
}

fn track_large_file(large_files: &mut LargeFilesHeap, path: &Path, size: u64) {
    large_files.push(Reverse((size, path.to_string_lossy().to_string())));

    if large_files.len() > 10 {
        large_files.pop();
    }
}

fn merge_largest_files(target: &mut LargeFilesHeap, source: LargeFilesHeap) {
    for item in source {
        target.push(item);
        if target.len() > 10 {
            target.pop();
        }
    }
}
