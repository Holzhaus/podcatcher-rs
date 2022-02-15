// Copyright (c) 2022 Jan Holthuis
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy
// of the MPL was not distributed with this file, You can obtain one at
// http://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Command line interface.

use crate::config::Config;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// A fictional versioning CLI
#[derive(Parser)]
#[clap(name = "podcaster")]
#[clap(about = "Simple tool to download podcast subscriptions", long_about = None)]
struct Cli {
    #[clap(required = false, long = "config", parse(from_os_str))]
    config: Option<PathBuf>,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show the current status.
    Status,
}

/// Main method.
pub fn main() {
    let args = Cli::parse();

    let config = if let Some(config_path) = &args.config {
        Config::from_path(config_path)
    } else {
        Config::from_default_path()
    }
    .unwrap();

    match &args.command {
        Commands::Status => {
            // TODO
            println!("Current Configuration: {:#?}", config);
        }
    }
}
