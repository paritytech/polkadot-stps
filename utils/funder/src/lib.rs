use sp_core::{sr25519::Pair as SrPair, Pair};

pub fn derive_accounts(n: usize, seed: String) -> Vec<SrPair> {
	let t = std::cmp::min(
		n,
		std::thread::available_parallelism().unwrap_or(1usize.try_into().unwrap()).get(),
	);

	let mut tn = (0..t).cycle();
	let mut tranges: Vec<_> = (0..t).map(|_| Vec::new()).collect();
	(0..n).for_each(|i| tranges[tn.next().unwrap()].push(i));
	let mut threads = Vec::new();

	tranges.into_iter().for_each(|chunk| {
		let seed = seed.clone();
		threads.push(std::thread::spawn(move || {
			chunk
				.into_iter()
				.map(move |i| {
					let derivation = format!("{seed}/{i}");
					<SrPair as Pair>::from_string(&derivation, None).unwrap()
				})
				.collect::<Vec<_>>()
		}));
	});

	threads
		.into_iter()
		.map(|h| h.join().unwrap())
		.flatten()
		.collect()
}
