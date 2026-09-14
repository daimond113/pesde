use std::num::NonZero;

use merkleberg::MMRIVER;
use pesde::source::pesde::registry::{CurrentMerkleHasher, LogHeadResponse, MmrAccumulator};
use pesde_registry_core::db::MmrReadStore;
use serde::Deserialize;

pub trait FromLogError: From<merkleberg::Error> + From<anyhow::Error> {}

#[derive(Debug, Deserialize)]
pub struct LogHeadQuery {
	from_size: Option<NonZero<u64>>,
}

pub async fn log_head<E: FromLogError>(
	mmr: MMRIVER<CurrentMerkleHasher, Box<dyn MmrReadStore>>,
	query: LogHeadQuery,
) -> Result<Option<LogHeadResponse>, E> {
	if mmr.mmr_size() == 0 {
		return Ok(None);
	}

	let proof_paths = match query.from_size {
		Some(from_size) => mmr
			.gen_consistency_proof(from_size.get())
			.await?
			.proof_paths()
			.to_vec(),
		None => Vec::new(),
	};

	Ok(Some(LogHeadResponse {
		accumulator: MmrAccumulator {
			peaks: mmr.get_accumulator().await?.into(),
		},
		mmr_size: mmr.mmr_size(),
		proof_paths,
	}))
}
