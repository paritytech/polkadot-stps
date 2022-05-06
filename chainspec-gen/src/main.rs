use std::process::Command;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::PathBuf;
use serde_json::{json, Value};
use structopt::StructOpt;
use subxt::{extrinsic::PairSigner, DefaultConfig,
            sp_core::{Pair, sr25519::Pair as SrPair}};

const VALID_CHAINS: &[&str] = &["kusama", "kusama-dev", "kusama-local", "kusama-staging", "polkadot", "polakdot-dev", "polkadot-local", "polkadot-staging", "rococo", "rococo-dev", "rococo-local", "rococo-staging", "westend", "westend-dev", "westend-local", "westend-staging", "wococo", "wococo-dev", "wococo-local", "versi", "versi-dev", "versi-local"];

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(short)]
    n: usize,

    #[structopt(short, long, possible_values(VALID_CHAINS))]
    chain: String,

    /// Relative or absolute path to the Polkadot executable.
    #[structopt(short, long, default_value = "../polkadot")]
    polkadot_binary: PathBuf,
}

fn main() {
    let opt = Opt::from_args();
    let chain = opt.chain.as_str();
    let n = opt.n;
    let polkadot_binary = fs::canonicalize(opt.polkadot_binary).expect("Unable to find the polkadot executable");

    let chainspec = chainspec(chain, polkadot_binary);
    let new_chainspec = modify_chainspec(chainspec, n);

    let mut file = File::create(format!("{}-funded.json", chain)).unwrap();
    file.write_all(&new_chainspec).unwrap();
}

fn chainspec(chain: &str, polkadot: PathBuf) -> Vec<u8> {
    Command::new(&polkadot)
        .arg("build-spec")
        .arg("--chain")
        .arg(chain)
        .output()
        .expect("failed to execute process")
        .stdout
}

const FUNDS: u64 = 10000000000000000;

fn modify_chainspec(chainspec: Vec<u8>, n: usize) -> Vec<u8> {
    let mut chainspec_json: Value = serde_json::from_slice(&chainspec).unwrap();
    
    // Extend the genesis balances with `//Sender/i`.
    let genesis_balances = chainspec_json["genesis"]["runtime"]["balances"]["balances"].as_array_mut().unwrap();
    for i in 0..n {
        let pair: SrPair = Pair::from_string(format!("//Sender/{}", i).as_str(), None).unwrap();
        let signer: PairSigner<DefaultConfig, SrPair> = PairSigner::new(pair);
        let a: (String, u64) = (signer.account_id().to_string(), FUNDS);
        let balance = serde_json::to_value(a).unwrap();
        genesis_balances.push(balance);
    }

    // erase bootnodes
    chainspec_json["bootNodes"] = json!([]);

    let new_chainspec = serde_json::to_vec_pretty(&chainspec_json).unwrap();
    new_chainspec
}
