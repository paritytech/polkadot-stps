use prometheus_endpoint::{prometheus::{Gauge, IntGauge, Opts}, Registry};
use std::net::ToSocketAddrs;

pub struct StpsMetrics {
    block_tps: Gauge,
    block_tx_count: IntGauge,
    block_time: IntGauge,
}

impl StpsMetrics {
    pub fn set(&self, tx_count: u64, block_time: u64) {
        self.block_tps.set((tx_count / block_time) as f64);
        self.block_tx_count.set(tx_count as i64);
        self.block_time.set(block_time as i64);
    }
}

pub async fn run_prometheus_endpoint(prometheus_url: &String, prometheus_port: &u16) -> anyhow::Result<StpsMetrics> {
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
           Gauge::with_opts(
               Opts::new(
                   "tps",
                   "Transactions per second in the block",
               )
           )?,
           &registry
        )?,
        block_tx_count: prometheus_endpoint::register(
           IntGauge::new(
               "tx_count",
               "Number of transactions in the block",
           )?,
           &registry
        )?,
        block_time: prometheus_endpoint::register(
           IntGauge::new(
               "block_time",
               "Block time delta in milliseconds",
           )?,
           &registry
        )?,
    })
}