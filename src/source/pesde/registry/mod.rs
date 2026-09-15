//! Data models for the registry

use std::convert::Infallible;
use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use jiff::Timestamp;
use merkle_bplustree::TreeConfig;
use merkle_bplustree::hasher::Hasher;
use merkleberg::Merge;
use semver::Prerelease;
use semver::Version;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::bounded::Bounded;
use crate::hash::Blake3Hash;
use crate::hash::Hash;
use crate::hash::Hasher as _;
use crate::names::LocalName;
use crate::ser_display_deser_fromstr;
use crate::signature::PublicKey;
use crate::signature::Signature;

mod global;
mod response;
mod scope;
pub use global::*;
pub use response::*;
pub use scope::*;

/// Returns a canonical serialisation of the given struct for cryptographic purposes
#[must_use]
pub fn canonical_bytes(data: &impl Serialize) -> Vec<u8> {
	cbor_core::Value::serialized(data)
		.expect("failed to serialise body for signing")
		.encode()
}

/// An entry in a log, at a known leaf position
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry<T> {
	/// The leaf position of this entry
	pub pos: u64,
	/// The time of publishing of this entry
	/// This value is server authoritative because of time sync issues a client provided value would pose
	pub published_at: Timestamp,
	/// The payload of this entry
	pub payload: T,
}

/// An object that carries its own signing key
pub trait WithSigner {
	/// The key that should sign this
	fn signer(&self) -> &PublicKey;
}

/// An unvalidated record carrying a signature and a signer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnvalidatedSigned<T: WithSigner> {
	/// The signature
	pub sig: Signature,
	/// The body
	#[serde(flatten)]
	pub body: T,
}

/// The signed entry was illegal in some way, e.g. the signature didn't match
#[derive(Debug, Error)]
#[error("the signed entry was illegal")]
pub struct SignedValidationFailed;

/// A validated wrapper over [UnvalidatedSigned], allowing construction only if it's legal
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct Signed<T: WithSigner>(UnvalidatedSigned<T>);

impl<T: WithSigner + Serialize> Signed<T> {
	/// Validates the passed in [UnvalidatedSigned] and returns Some if it's valid
	pub fn new(input: UnvalidatedSigned<T>) -> Result<Self, SignedValidationFailed> {
		if !input
			.sig
			.verify(input.body.signer(), &canonical_bytes(&input.body))
		{
			return Err(SignedValidationFailed);
		}

		Ok(Self(input))
	}

	/// Returns the underlying [UnvalidatedSigned]
	pub fn into_inner(self) -> UnvalidatedSigned<T> {
		self.0
	}
}

impl<'de, T: WithSigner + Serialize + Deserialize<'de>> Deserialize<'de> for Signed<T> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Self::new(UnvalidatedSigned::deserialize(deserializer)?).map_err(serde::de::Error::custom)
	}
}

/// The scope id; hash of the scope name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScopeId(CurrentHash);

impl ScopeId {
	/// Creates a new [Self] from a [Scope](crate::names::Scope)
	#[must_use]
	pub fn from(name: &crate::names::Scope) -> Self {
		let mut hasher = CurrentHash::hasher();
		hasher.update(name.as_str().as_bytes());
		Self(hasher.finalize())
	}
}

/// The local name id; hash of the package local name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct LocalNameId(CurrentHash);

impl LocalNameId {
	/// Creates a new [Self] from a [LocalName](crate::names::LocalName)
	#[must_use]
	pub fn from(name: &crate::names::LocalName) -> Self {
		let mut hasher = CurrentHash::hasher();
		hasher.update(name.as_str().as_bytes());
		Self(hasher.finalize())
	}
}

/// The current hash used by the registry
pub type CurrentHash = Blake3Hash;

/// The [Merge] and [Hasher] implementation using the [CurrentHash]
#[derive(Debug)]
pub struct CurrentMerkleHasher;

impl Merge for CurrentMerkleHasher {
	type Item = CurrentHash;
	type Error = Infallible;

	fn leaf_hash(data: &[u8]) -> Result<Self::Item, Self::Error> {
		let mut hasher = CurrentHash::hasher();
		hasher.update(&[0x00]);
		hasher.update(data);
		Ok(hasher.finalize())
	}

	fn merge_pos(
		pos: u64,
		left: &Self::Item,
		right: &Self::Item,
	) -> Result<Self::Item, Self::Error> {
		let mut hasher = CurrentHash::hasher();
		hasher.update(&[0x01]);
		hasher.update(&pos.to_be_bytes());
		hasher.update(left.0.as_ref());
		hasher.update(right.0.as_ref());
		Ok(hasher.finalize())
	}
}

impl<K: Serialize, V: Serialize> Hasher<K, V> for CurrentMerkleHasher {
	type Output = CurrentHash;

	fn empty_hash() -> Self::Output {
		let mut hasher = CurrentHash::hasher();
		hasher.update(&[0x10]);
		hasher.finalize()
	}

	fn hash_key(key: &K) -> Self::Output {
		let mut hasher = CurrentHash::hasher();
		hasher.update(&[0x11]);
		cbor_core::Value::serialized(key)
			.unwrap()
			.write_to(&mut hasher)
			.unwrap();
		hasher.finalize()
	}

	fn hash_value(value: &V) -> Self::Output {
		let mut hasher = CurrentHash::hasher();
		hasher.update(&[0x12]);
		cbor_core::Value::serialized(value)
			.unwrap()
			.write_to(&mut hasher)
			.unwrap();
		hasher.finalize()
	}

	fn hash_slot(key: &K, child: &Self::Output) -> Self::Output {
		let mut hasher = CurrentHash::hasher();
		hasher.update(&[0x13]);
		cbor_core::Value::serialized(key)
			.unwrap()
			.write_to(&mut hasher)
			.unwrap();
		hasher.update(child.0.as_ref());
		hasher.finalize()
	}

	fn merge_hashes(a: &Self::Output, b: &Self::Output) -> Self::Output {
		let mut hasher = CurrentHash::hasher();
		hasher.update(a.0.as_ref());
		hasher.update(b.0.as_ref());
		hasher.finalize()
	}

	fn hash_leaf(entry_count: usize, merkle_root: &Self::Output) -> Self::Output {
		let mut hasher = CurrentHash::hasher();
		hasher.update(&[0x14]);
		hasher.update(&(entry_count as u64).to_be_bytes());
		hasher.update(merkle_root.0.as_ref());
		hasher.finalize()
	}

	fn hash_internal(child_count: usize, slots_root: &Self::Output) -> Self::Output {
		let mut hasher = CurrentHash::hasher();
		hasher.update(&[0x15]);
		hasher.update(&(child_count as u64).to_be_bytes());
		hasher.update(slots_root.0.as_ref());
		hasher.finalize()
	}
}
