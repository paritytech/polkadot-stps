use std::process::Command;
use std::env;
use std::fs::File;
use std::io::prelude::*;
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
}

fn main() {
    let opt = Opt::from_args();
    let chain = opt.chain.as_str();
    let n = opt.n;

    let chainspec = chainspec(chain);
    let new_chainspec = modify_chainspec(chainspec, n);

    let mut file = File::create(format!("{}-funded.json", chain)).unwrap();
    file.write_all(&new_chainspec).unwrap();
}

fn chainspec(chain: &str) -> Vec<u8> {
    let current_dir = env::current_dir().unwrap();
    let mut above_dir = current_dir.clone();
    above_dir.pop();

    let polkadot_cmd = above_dir.to_str().unwrap().to_owned() + "/polkadot";

    let output = Command::new(polkadot_cmd)
        .arg("build-spec")
        .arg("--chain")
        .arg(chain)
        .output()
        .expect("failed to execute process");

    output.stdout
}

fn modify_chainspec(chainspec: Vec<u8>, n: usize) -> Vec<u8> {
    let mut chainspec_json: Value = serde_json::from_slice(&chainspec).unwrap();
    let mut v = Vec::new();
    for i in 0..n {
        let pair: SrPair = Pair::from_string(format!("//Sender-{}", i).as_str(), None).unwrap();
        let signer: PairSigner<DefaultConfig, SrPair> = PairSigner::new(pair);
        let a: (String, i64) = (signer.account_id().to_string(), 10000000000000000);
        v.push(a);
    }
    let balances = serde_json::to_value(&v).unwrap();

    // replace sender accounts into genesis
    chainspec_json["genesis"]["runtime"]["balances"]["balances"] = balances;

    // erase bootnodes
    chainspec_json["bootNodes"] = json!([]);

    let new_chainspec = serde_json::to_vec_pretty(&chainspec_json).unwrap();
    new_chainspec
}