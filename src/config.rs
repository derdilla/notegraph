use std::path::PathBuf;

use clap::{command, Parser};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Config {

    #[arg(short, long)]
    pub input_dir: String
}