use prometheus_endpoint::{
	prometheus::{GaugeVec, IntGaugeVec, Opts},
	Registry,
};
use std::net::ToSocketAddrs;

pub struct StpsMetrics {
	block_tps: GaugeVec,
	block_tx_count: IntGaugeVec,
	block_time: IntGaugeVec,
}

impl StpsMetrics {
	pub fn set(&self, tx_count: u64, block_time: u64, block_number: u64) {
		self.block_tps
			.with_label_values(&[&block_number.to_string()])
			.set(tx_count as f64 / block_time as f64);
		self.block_time
			.with_label_values(&[&block_number.to_string()])
			.set(block_time as i64);
		self.block_tx_count
			.with_label_values(&[&block_number.to_string()])
			.set(tx_count as i64);
	}
}

pub async fn run_prometheus_endpoint(
	prometheus_url: &String,
	prometheus_port: &u16,
) -> anyhow::Result<StpsMetrics> {
	let registry = Registry::new_custom(Some("sTPS".into()), None)?;
	let metrics = register_metrics(&registry)?;
	let socket_addr_str = format!("{}:{}", prometheus_url, prometheus_port);
	for addr in socket_addr_str.to_socket_addrs()? {
		let prometheus_registry = registry.clone();
		tokio::spawn(prometheus_endpoint::init_prometheus(addr, prometheus_registry));
	}

	Ok(metrics)
}

fn register_metrics(registry: &Registry) -> anyhow::Result<StpsMetrics> {
	Ok(StpsMetrics {
		block_tps: prometheus_endpoint::register(
			GaugeVec::new(
				Opts::new("tps", "Transactions per second in the block"),
				&["block_number"],
			)?,
			&registry,
		)?,
		block_tx_count: prometheus_endpoint::register(
			IntGaugeVec::new(
				Opts::new("tx_count", "Number of transactions in the block"),
				&["block_number"],
			)?,
			&registry,
		)?,
		block_time: prometheus_endpoint::register(
			IntGaugeVec::new(
				Opts::new("block_time", "Block time delta in milliseconds"),
				&["block_number"],
			)?,
			&registry,
		)?,
	})
}
