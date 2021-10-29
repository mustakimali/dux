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
23.56 MB        {file1}

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
For smaller directory, the performance is on-par or better than `du` (I have no idea why!)

```bash
$ time du -hd 0
126G    .
du -hd 0  0.01s user 0.04s system 47% cpu 0.088 total

$ time dux
Total size is 129930609382 bytes (129.93 GB) across 159 items
dux  0.00s user 0.01s system 51% cpu 0.024 total
```

but for larger folder `du` is `3x` faster at least,

```bash
$ time du -hd 0
699M    .
du -hd 0  0.05s user 0.67s system 48% cpu 1.498 total

$ time dux     
Total size is 666559986 bytes (666.56 MB) across 31623 items
dux  0.21s user 3.16s system 76% cpu 4.390 total

```

## Suggestions

If you have any idea about how the performance can be improved (without writing unsafe code) and anything to make the code more idiomatic - then help this friend out by sending PR.