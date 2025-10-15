mod cli;
mod core;
mod web;

use anyhow::Context;
use clap::Parser;
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

#[derive(clap::Subcommand, Clone)]
enum Commands {
    /// Start a web UI for server management
    Server,
    #[command(subcommand)]
    /// Manage server using command line
    Manage(Manage),
}

#[derive(clap::Subcommand, Clone)]
enum Manage {
    #[command(subcommand)]
    /// Manage users
    User(User),
    #[command(subcommand)]
    /// Manage worlds
    World(World),
}

#[derive(clap::Subcommand, Clone)]
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

#[derive(clap::Subcommand, Clone)]
enum World {
    /// List all available worlds
    List,
    /// Switch the active world
    Switch {
        /// The name of the world to switch to
        world_name: String,
    },
}

fn real_main(args: Args) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_thread_names(true)
        .with_line_number(true)
        .with_level(true)
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .expect("Failed to configure the logger");

    let config =
        core::Config::load(args.config).with_context(|| "Failed to load configuration file")?;

    match Args::parse().cmd {
        Commands::Server => web::start_server(config).with_context(|| "Web server has failed"),
        Commands::Manage(command_type) => match command_type {
            Manage::World(world) => match world {
                World::List => cli::world::list(config.app_config)
                    .with_context(|| "Failed to get the list of available worlds"),
                World::Switch { world_name } => {
                    cli::world::switch(config.app_config, world_name).map_err(|err| err.into())
                }
            },
            Manage::User(user_command) => match user_command {
                User::Enroll { username } => cli::user::enroll(config.app_config, username)
                    .with_context(|| "Failed to enroll a new user"),
                User::Remove { username } => cli::user::remove(config.app_config, username)
                    .with_context(|| "Failed to remove a new user"),
            },
        },
    }
}

fn main() {
    let args = Args::parse();

    if let Err(err) = real_main(args) {
        tracing::error!(
            "{}",
            err.chain()
                .map(|err| err.to_string())
                .collect::<Vec<_>>()
                .join(": ")
        );

        std::process::exit(1);
    }
}
