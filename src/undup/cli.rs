mod actions;
mod hashing;

use clap::Parser;
use std::{io, path::PathBuf};

#[derive(Parser, Debug)]
struct Arguments {
    #[clap(short, long, value_parser, required = true)]
    source_paths: Vec<PathBuf>,
    #[clap(short, long, value_parser, required = true)]
    target_paths: Vec<PathBuf>,

    #[clap(long, short)]
    dry_run: bool,
}

fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().filter_or("ATORR_LOG", "warn"));
    let args = Arguments::parse();

    let matching_files = hashing::find_matching_files(&args.source_paths, &args.target_paths)?;
    if args.dry_run {
        actions::dry_run(&matching_files);
    } else {
        actions::symlink_matching_files(&matching_files)?;
    }

    Ok(())
}
