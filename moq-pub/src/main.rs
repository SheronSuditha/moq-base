use anyhow::Context;
use clap::Parser;
use tokio::task::JoinSet;

mod session_runner;
use session_runner::*;

mod media_runner;
use media_runner::*;

mod log_viewer;
use log_viewer::*;

mod media;
use media::*;

mod cli;
use cli::*;

// TODO: clap complete

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	let config = Config::parse();

	let mut media = Media::new(&config).await?;
	let session_runner = SessionRunner::new(&config).await?;
	let mut log_viewer = LogViewer::new(session_runner.get_incoming_receivers().await).await?;
	let mut media_runner = MediaRunner::new(
		session_runner.get_send_objects().await,
		session_runner.get_outgoing_senders().await,
		session_runner.get_incoming_receivers().await,
	)
	.await?;

	let mut join_set: JoinSet<anyhow::Result<()>> = tokio::task::JoinSet::new();

	join_set.spawn(async { session_runner.run().await.context("failed to run session runner") });
	join_set.spawn(async move { log_viewer.run().await.context("failed to run media source") });

	// TODO: generate unique namespace with UUID and/or take a command line arg
	media_runner.announce("quic.video/moq-pub-foo", media.source()).await?;

	join_set.spawn(async move { media.run().await.context("failed to run media source") });
	join_set.spawn(async move { media_runner.run().await.context("failed to run client") });

	while let Some(res) = join_set.join_next().await {
		dbg!(&res);
		res??;
	}

	Ok(())
}
