# dux
A Rust implementation of GNU coreutils `du` in order to learn and explore Rust. This is by no means better, faster or smarter compared to `du`. So don't use it.

## Build
```bash
# build
$ cargo build --release

# build and publish to ~/bin/ folder
$ ./publish
```

## Usage

```bash
# current path
$ dux
Total size is 204828703 bytes (204.83 MB) across 1176 items

# specify a path
$ dux ~/bin/
Total size is 586934311 bytes (586.93 MB) across 3372 items

# list largest files
$ dux -l
Largest files:
+----------+----------------+
| Size     | File           |
+----------+----------------+
| 23.56 MB | {file 1}       |
+----------+----------------+

Total size is 586934311 bytes (586.93 MB) across 3372 items
```

### Cli Args
```
dux 0.2.0

Calculate disk space used by a folder

USAGE:
    dux [OPTIONS] [PATH]

ARGS:
    <PATH>    The folder to use (default to current directory)

OPTIONS:
    -h, --help                Print help information
    -l, --list-large-files    Lists top 10 largest files
    -V, --version             Print version information
```

## Performance
See [Benchmark](BENCH.md)

## Suggestions

If you have any idea about how the performance can be improved (without writing unsafe code) and anything to make the code more idiomatic - then help this friend out by sending PR.
