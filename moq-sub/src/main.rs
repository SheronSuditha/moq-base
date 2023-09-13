use anyhow::{Context, Result};
use log::{debug, Level};
use quinn;
use rustls::{Certificate, RootCertStore};
use rustls_native_certs;
use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;
use webtransport_quinn;

mod cli;

#[tokio::main]
async fn main() -> Result<()> {
	let log_level = Level::Debug; // Change this to the desired log level

	// Create a Config object with the desired server URI and port
	let bind_address = "[::]:0".parse().unwrap();
	let uri = "https://192.168.4.1:4443".parse().unwrap();

	// Load platform certificates
	let mut roots = rustls::RootCertStore::empty();
	for cert in rustls_native_certs::load_native_certs().context("could not load platform certs")? {
		roots.add(&Certificate(cert.0)).context("could not add certificate")?;
	}

	// Configure the TLS client
	let mut tls_config = rustls::ClientConfig::builder()
		.with_safe_defaults()
		.with_root_certificates(roots)
		.with_no_client_auth();

	tls_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()];

	let arc_tls_config = std::sync::Arc::new(tls_config);
	let quinn_client_config = quinn::ClientConfig::new(arc_tls_config);

	let mut endpoint = quinn::Endpoint::client(bind_address)?;
	endpoint.set_default_client_config(quinn_client_config);

	// Create a WebTransport session
	let webtransport_session = webtransport_quinn::connect(&endpoint, &uri)
		.await
		.context("failed to create WebTransport session")?;

	let moq_transport_session = moq_transport::Session::connect(webtransport_session, moq_transport::setup::Role::Both)
		.await
		.context("failed to create MoQ Transport session")?;

	// send a subscribe message
	let mut send_control = moq_transport_session.send_control;
	let mut recv_control = moq_transport_session.recv_control;

	let trackid = moq_transport::VarInt::from_u32(1);

	let subscribe = moq_transport::message::Subscribe {
		track_namespace: "quic.video/moq-pub-foo".to_string(),
		track_id: trackid,
		track_name: "1".to_string(),
	};

	send_control
		.send(moq_transport::message::Message::Subscribe(subscribe))
		.await?;

	let mut recv_object = moq_transport_session.recv_objects;
	let mut join_set: JoinSet<anyhow::Result<()>> = tokio::task::JoinSet::new();

	// size_limit
	let size_limit = 150000000;

	// join_set.spawn(async move {
	// 	loop {
	// 		let (object, mut _stream) = recv_object.recv().await?;
	// 		// println!("Received object: {:?}", object);
	// 		println!("Received object: {:?}", _stream.read_to_end(size_limit).await?);
	// 	}
	// });
	// wait for the go ahead
	loop {
		match recv_control.recv().await? {
			moq_transport::message::Message::SubscribeOk(_) => {
				println!("Received SubscribeOk");

				// break;
			}
			moq_transport::message::Message::SubscribeError(subscribe_error) => {
				debug!(
					"Failed to subscribe to track '{}' with error code '{}' and reason '{}'",
					"test", &subscribe_error.code, &subscribe_error.reason
				);
				return Ok(());
			}
			_ => {}
		}
	}

	Ok(())
}
