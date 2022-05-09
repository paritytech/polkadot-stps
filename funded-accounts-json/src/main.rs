use std::fs::File;
use std::io::prelude::*;
use structopt::StructOpt;
use subxt::{extrinsic::PairSigner, DefaultConfig,
            sp_core::{Pair, sr25519::Pair as SrPair}};

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(short)]
    n: usize,
}

fn main() {
    let opt = Opt::from_args();
    let n = opt.n;

    let funded_accounts_json = funded_accounts_json(n);
    
    let mut file = File::create("funded-accounts.json").unwrap();
    file.write_all(&funded_accounts_json).unwrap();
}

const FUNDS: u64 = 10000000000000000;

fn funded_accounts_json(n: usize) -> Vec<u8> {
    let mut v = Vec::new();
    for i in 0..n {
        let pair: SrPair = Pair::from_string(format!("//Sender/{}", i).as_str(), None).unwrap();
        let signer: PairSigner<DefaultConfig, SrPair> = PairSigner::new(pair);
        let a: (String, u64) = (signer.account_id().to_string(), FUNDS);
        v.push(a);
    }

    let v_json = serde_json::to_value(&v).unwrap();
    serde_json::to_vec_pretty(&v_json).unwrap()
}
