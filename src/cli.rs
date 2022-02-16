// Copyright (c) 2022 Jan Holthuis
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy
// of the MPL was not distributed with this file, You can obtain one at
// http://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Command line interface.

use crate::config::Config;
use crate::download::{download_file, fetch_sync_info, to_human_size, EpisodeDownload};
use clap::{Parser, Subcommand};
use futures::lock::Mutex;
use futures::stream::StreamExt;
use std::path::PathBuf;

/// A fictional versioning CLI
#[derive(Debug, Parser)]
#[clap(name = "podcaster")]
#[clap(about = "Simple tool to download podcast subscriptions", long_about = None)]
struct Cli {
    #[clap(required = false, long = "config", parse(from_os_str))]
    config: Option<PathBuf>,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand, PartialEq)]
enum Commands {
    /// Show the current status.
    Status,
    /// Fetch the latest podcasts.
    Sync,
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

    if !config.download_dir.is_dir() {
        println!(
            "Download directory does not exist: {:?}",
            config.download_dir
        );
        return;
    }

    let max_jobs = config.max_parallel_downloads.unwrap_or(5usize);
    let files_to_download: Vec<EpisodeDownload> =
        fetch_sync_info(config.download_dir, config.podcast, max_jobs).await;

    println!();
    if files_to_download.is_empty() {
        println!("Nothing to do.");
        return;
    }

    let (total_size, is_partial) =
        files_to_download
            .iter()
            .fold((0, false), |(total_size, is_partial), file| {
                let size = file.file_size.unwrap_or(0);
                (total_size + size, is_partial || size == 0)
            });

    let (human_size, human_size_suffix) = to_human_size(total_size);
    if is_partial {
        println!(
            "Total Download Size: {}{} (size of some files is unknown)",
            human_size, human_size_suffix
        );
    } else {
        println!("Total Download Size: {}{}", human_size, human_size_suffix);
    }
    println!();

    if args.command == Commands::Status {
        println!("Files to download:");
        for file in &files_to_download {
            println!(
                "  {} ({}, {})",
                file.file_name(),
                file.url,
                file.human_file_size()
            );
        }
        return;
    }

    println!("Fetching audio files...");
    let progress = std::sync::Arc::new(Mutex::new(linya::Progress::new()));
    let task_count = files_to_download.len();
    futures::stream::iter(files_to_download.into_iter())
        .enumerate()
        .for_each_concurrent(max_jobs, move |(i, dl)| {
            let prog = progress.clone();
            async move {
                std::fs::create_dir_all(&dl.file_path.parent().unwrap()).unwrap();
                let mut data = std::fs::File::create(&dl.file_path).unwrap();
                download_file(
                    &mut data,
                    prog.clone(),
                    &dl.url,
                    dl.file_size,
                    format!("({}/{}) {}", i + 1, &task_count, dl.file_name()).as_ref(),
                )
                .await
                .unwrap();
            }
        })
        .await;
}
