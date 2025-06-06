// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, time::Instant};

use prometheus::Registry;
use sui_indexer_alt_framework::{
    ingestion::ClientArgs,
    postgres::{reset_database, DbArgs},
    IndexerArgs,
};
use sui_indexer_alt_schema::MIGRATIONS;
use sui_synthetic_ingestion::synthetic_ingestion::read_ingestion_data;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{config::IndexerConfig, setup_indexer};

#[derive(clap::Args, Debug, Clone)]
pub struct BenchmarkArgs {
    /// Path to the local ingestion directory to read checkpoints data from.
    #[arg(long)]
    ingestion_path: PathBuf,

    /// Only run the following pipelines. If not provided, all pipelines found in the
    /// configuration file will be run.
    #[arg(long, action = clap::ArgAction::Append)]
    pipeline: Vec<String>,
}

pub async fn run_benchmark(
    database_url: Url,
    db_args: DbArgs,
    benchmark_args: BenchmarkArgs,
    indexer_config: IndexerConfig,
) -> anyhow::Result<()> {
    let BenchmarkArgs {
        ingestion_path,
        pipeline,
    } = benchmark_args;

    let ingestion_data = read_ingestion_data(&ingestion_path).await?;
    let first_checkpoint = *ingestion_data.keys().next().unwrap();
    let last_checkpoint = *ingestion_data.keys().last().unwrap();
    let num_transactions: usize = ingestion_data.values().map(|c| c.transactions.len()).sum();

    reset_database(database_url.clone(), db_args.clone(), Some(&MIGRATIONS)).await?;

    let indexer_args = IndexerArgs {
        first_checkpoint: Some(first_checkpoint),
        last_checkpoint: Some(last_checkpoint),
        pipeline,
        ..Default::default()
    };

    let client_args = ClientArgs {
        remote_store_url: None,
        local_ingestion_path: Some(ingestion_path.clone()),
        rpc_api_url: None,
        rpc_username: None,
        rpc_password: None,
    };

    let cur_time = Instant::now();

    setup_indexer(
        database_url,
        db_args,
        indexer_args,
        client_args,
        indexer_config,
        false, /* with_genesis */
        &Registry::new(),
        CancellationToken::new(),
    )
    .await?
    .run()
    .await?
    .await?;

    let elapsed = Instant::now().duration_since(cur_time);
    println!("Indexed {} transactions in {:?}", num_transactions, elapsed);
    println!("TPS: {}", num_transactions as f64 / elapsed.as_secs_f64());
    Ok(())
}
