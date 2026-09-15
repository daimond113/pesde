use crate::source::pesde::registry::*;
use serde::{Deserialize, Serialize};

/// A MMR accumulator
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmrAccumulator {
	/// The peak hashes
	pub peaks: Arc<[CurrentHash]>,
}

/// The response of a log head endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct LogHeadResponse {
	/// The accumulator of the log
	pub accumulator: MmrAccumulator,
	/// The MMR's current size
	pub mmr_size: u64,
	/// The consistency proof paths
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub consistency_proof: Vec<Vec<<CurrentMerkleHasher as Merge>::Item>>,
}

/// The respones of a log entry endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntryResponse<P> {
	/// The entry
	pub entry: Entry<P>,
	/// The proof path from the entry to the peaks, included if `at_size` is specified in the query
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub inclusion_proof: Vec<<CurrentMerkleHasher as Merge>::Item>,
}
