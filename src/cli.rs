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
use futures::StreamExt;
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
#[tokio::main]
pub async fn main() {
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

    let max_jobs = config.max_parallel_downloads.unwrap_or(5usize);
    println!("Using {} parallel downloads.", max_jobs);

    let _results: Vec<Result<rss::Channel, Box<dyn std::error::Error>>> =
        futures::stream::iter(config.podcast.iter())
            .map(|podcast| async move {
                println!("Fetching {:?}", podcast.feed_url);

                let content = reqwest::get(&podcast.feed_url).await?.bytes().await?;
                let feed = rss::Channel::read_from(&content[..])?;
                println!("{} ({}, {} items)", feed.title, feed.link, feed.items.len());
                Ok(feed)
            })
            .buffer_unordered(max_jobs)
            .collect()
            .await;
}
