mod actions;
mod hashing;
mod matching;

use clap::Parser;
use std::{io, path::PathBuf};

use crate::hashing::no_cache::HashingNoCache;

#[derive(Clone, Debug, clap::ValueEnum)]
enum HashingCacheOptions {
    NoCache,
    Sqlite
}

#[derive(Parser, Debug)]
struct Arguments {
    #[clap(short, long, value_parser, required = true)]
    source_paths: Vec<PathBuf>,
    #[clap(short, long, value_parser, required = true)]
    target_paths: Vec<PathBuf>,
    #[clap(long, value_enum, default_value_t=HashingCacheOptions::Sqlite )]
    hashing_cache: HashingCacheOptions,

    #[clap(long, short)]
    dry_run: bool,
}

fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().filter_or("ATORR_LOG", "warn"));
    let args = Arguments::parse();

    let hasher = match args.hashing_cache {
        HashingCacheOptions::NoCache => HashingNoCache {},
        HashingCacheOptions::Sqlite => todo!(),
    };

    let matching_files = matching::find_matching_files(&args.source_paths, &args.target_paths, &hasher)?;
    if args.dry_run {
        actions::dry_run(&matching_files);
    } else {
        actions::symlink_matching_files(&matching_files)?;
    }

    Ok(())
}
