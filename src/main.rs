mod cli;
mod core;
mod web;

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path;

#[derive(Parser)]
#[command()]
struct Args {
    #[command(subcommand)]
    cmd: Commands,
    #[arg(short, long, default_value = "mctrlrs.toml")]
    /// Path to the YAML configuration file. If absolute path is provided it will be used as is.
    /// The relative path starting from "./" or "../" will be resolved using current working
    /// directory as a base path. The relative path that starts from something other than "./" or "../" will be resolved against the binary location.
    config: path::PathBuf,
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Start a web UI for server management
    Server,
    #[command(subcommand)]
    /// Manage server using command line
    Manage(Manage),
}

#[derive(Subcommand, Clone)]
enum Manage {
    #[command(subcommand)]
    /// Manage users
    User(User),
    /// Manage worlds
    World,
}

#[derive(Subcommand, Clone)]
enum User {
    /// Enroll a new user into the system
    Enroll {
        /// The username for a new user
        username: String,
    },
    /// Remove a user from the system
    Remove {
        /// The username of the user to remove
        username: String,
    },
}

fn real_main(args: Args) -> anyhow::Result<()> {
    let config =
        core::Config::load(args.config).with_context(|| "Failed to load configuration file")?;

    match Args::parse().cmd {
        Commands::Server => web::start_server(config).with_context(|| "Web server has failed"),
        Commands::Manage(command_type) => match command_type {
            Manage::World => todo!(),
            Manage::User(user_command) => match user_command {
                User::Enroll { username } => cli::user_enroll(config.app_config, username)
                    .with_context(|| "Failed to enroll a new user"),
                User::Remove { username } => cli::user_remove(config.app_config, username)
                    .with_context(|| "Failed to remove a new user"),
            },
        },
    }
}

fn main() {
    let args = Args::parse();

    if let Err(err) = real_main(args) {
        eprintln!("{err:?}");
        std::process::exit(1);
    }
}
