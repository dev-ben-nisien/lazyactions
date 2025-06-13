use std::process::Command;

use clap::Parser;
use color_eyre::eyre::eyre;

use crate::app::App;

pub mod app;
pub mod event;
pub mod gh_cli;
pub mod ui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Filter for current branch
    #[arg(short, long)]
    branch: bool,

    /// Filter for current user
    #[arg(short, long)]
    user: bool,

    /// Lastest Run Only
    #[arg(short, long)]
    latest: bool,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let _args = Args::parse();
    Command::new("clear");
    // Check for GitHub CLI installation and authentication
    println!("Checking GitHub CLI status...");
    let auth_status = Command::new("gh").arg("auth").arg("status").output()?;

    if !auth_status.status.success() {
        return Err(eyre!(
            "GitHub CLI is not installed or not authenticated. Please install it and run 'gh auth login'."
        ));
    }
    println!("GitHub CLI is installed and authenticated.");
    let terminal = ratatui::init();
    let result = App::new().run(terminal);
    ratatui::restore();
    result
}
