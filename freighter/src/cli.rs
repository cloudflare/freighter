use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct FreighterArgs {
    /// Path to the config file.
    #[arg(short, long)]
    pub config: PathBuf,
}
