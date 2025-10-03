mod actions;
mod hashing;
mod matching;

use clap::Parser;
use directories::ProjectDirs;
use std::{io, path::PathBuf};

use crate::hashing::{HashCache, file_cache::HashingFileCache, no_cache::HashingNoCache};

#[derive(Clone, Debug, clap::ValueEnum)]
enum HashingCacheOptions {
    NoCache,
    File,
}

#[derive(Parser, Debug)]
struct Arguments {
    #[clap(short, long, value_parser, required = true)]
    source_paths: Vec<PathBuf>,
    #[clap(short, long, value_parser, required = true)]
    target_paths: Vec<PathBuf>,
    #[clap(long, value_enum, default_value_t=HashingCacheOptions::File )]
    hashing_cache: HashingCacheOptions,

    #[clap(long, short)]
    dry_run: bool,
}

fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().filter_or("ATORR_LOG", "warn"));
    let args = Arguments::parse();

    let dirs: directories::ProjectDirs =
        directories::ProjectDirs::from("local", "jimbo", "untorr_undup")
            .expect("Could not find the project directories");
    create_dirs(&dirs)?;

    let mut hasher: Box<dyn HashCache> = match args.hashing_cache {
        HashingCacheOptions::NoCache => Box::new(HashingNoCache::new()),
        HashingCacheOptions::File => {
            Box::new(HashingFileCache::new(dirs.cache_dir().join("hashes.cache")).unwrap())
        }
    };

    let matching_files =
        matching::find_matching_files(&args.source_paths, &args.target_paths, hasher.as_mut())?;
    if args.dry_run {
        actions::dry_run(&matching_files);
    } else {
        actions::symlink_matching_files(&matching_files)?;
    }

    Ok(())
}

fn create_dirs(dirs: &ProjectDirs) -> io::Result<()> {
    std::fs::create_dir_all(dirs.cache_dir())?;
    Ok(())
}
