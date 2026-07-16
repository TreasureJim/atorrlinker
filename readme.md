# atorrlinker-undup

A Rust tool for finding duplicate files and replacing them with hard links.

## Description

This utility scans directories to identify duplicate files based on their content. When duplicates are found, it replaces them with hard links to a single copy, saving disk space.

## Running

Build the project using Cargo:

```bash
cargo build --release
```

The executable will be located at `target/release/atorrlinker-undup`.

Otherwise one can run (with program arguments after the two dashes):

```bash
cargo run -- 
```

## Usage

Run the tool from the command line, specifying the target directory:

```bash
./atorrlinker-undup /path/to/directory
```

The tool will scan the specified directory, identify duplicate files, and replace them with hard links.
