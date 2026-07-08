//! CLI argument parsing.

use crate::model::Category;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "mac-cleaner",
    version,
    about = "Fast, safe macOS storage cleaner with a TUI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Skip automatic sudo elevation at startup.
    #[arg(long, global = true)]
    pub no_elevate: bool,

    /// Config file path.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Dry run — report actions without deleting.
    #[arg(long, global = true)]
    pub dry_run: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the interactive TUI (default).
    Tui,
    /// Scan and print results.
    Scan {
        /// Output JSON instead of a table.
        #[arg(long)]
        json: bool,
        /// Comma-separated categories: caches,logs,dev,duplicates,icloud,large,trash
        #[arg(long)]
        categories: Option<String>,
    },
    /// Headless clean.
    Clean {
        #[arg(long)]
        categories: String,
        /// Skip confirmation.
        #[arg(long)]
        yes: bool,
        /// Permanently delete instead of moving to Trash.
        #[arg(long)]
        permanent: bool,
    },
    /// Write the default config to ~/.config/mac-cleaner/config.toml.
    InitConfig,
}

impl Cli {
    pub fn parse_categories(s: &str) -> Vec<Category> {
        s.split(',')
            .filter_map(|p| Category::from_slug(p.trim()))
            .collect()
    }
}
