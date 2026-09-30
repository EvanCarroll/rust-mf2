//! Counts the files in a directory, and says so in the reader's language.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

mf2::include_generated!();

#[derive(Parser)]
struct Args {
    /// The directory to count.
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// The language to answer in, instead of the system's.
    #[arg(long)]
    lang: Option<Locale>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    install();
    if let Some(lang) = args.lang {
        set_locale(lang);
    }
    match std::fs::read_dir(&args.dir) {
        Ok(entries) => {
            println!("{}", tr!("files", dir = &args.dir, count = entries.count()));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", tr!("unreadable", dir = &args.dir, error = error));
            ExitCode::FAILURE
        }
    }
}
