// Copyright 2017-2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Substrate blockchain trait

use std::sync::Arc;

use sr_primitives::traits::{Block as BlockT, Header as HeaderT, NumberFor};
use sr_primitives::generic::BlockId;
use sr_primitives::Justification;

use crate::error::{Error, Result};

/// Blockchain database header backend. Does not perform any validation.
pub trait HeaderBackend<Block: BlockT>: Send + Sync {
	/// Get block header. Returns `None` if block is not found.
	fn header(&self, id: BlockId<Block>) -> Result<Option<Block::Header>>;
	/// Get blockchain info.
	fn info(&self) -> Info<Block>;
	/// Get block status.
	fn status(&self, id: BlockId<Block>) -> Result<BlockStatus>;
	/// Get block number by hash. Returns `None` if the header is not in the chain.
	fn number(&self, hash: Block::Hash) -> Result<Option<<<Block as BlockT>::Header as HeaderT>::Number>>;
	/// Get block hash by number. Returns `None` if the header is not in the chain.
	fn hash(&self, number: NumberFor<Block>) -> Result<Option<Block::Hash>>;

	/// Convert an arbitrary block ID into a block hash.
	fn block_hash_from_id(&self, id: &BlockId<Block>) -> Result<Option<Block::Hash>> {
		match *id {
			BlockId::Hash(h) => Ok(Some(h)),
			BlockId::Number(n) => self.hash(n),
		}
	}

	/// Convert an arbitrary block ID into a block hash.
	fn block_number_from_id(&self, id: &BlockId<Block>) -> Result<Option<NumberFor<Block>>> {
		match *id {
			BlockId::Hash(_) => Ok(self.header(*id)?.map(|h| h.number().clone())),
			BlockId::Number(n) => Ok(Some(n)),
		}
	}

	/// Get block header. Returns `UnknownBlock` error if block is not found.
	fn expect_header(&self, id: BlockId<Block>) -> Result<Block::Header> {
		self.header(id)?.ok_or_else(|| Error::UnknownBlock(format!("{}", id)))
	}

	/// Convert an arbitrary block ID into a block number. Returns `UnknownBlock` error if block is not found.
	fn expect_block_number_from_id(&self, id: &BlockId<Block>) -> Result<NumberFor<Block>> {
		self.block_number_from_id(id)
			.and_then(|n| n.ok_or_else(|| Error::UnknownBlock(format!("{}", id))))
	}

	/// Convert an arbitrary block ID into a block hash. Returns `UnknownBlock` error if block is not found.
	fn expect_block_hash_from_id(&self, id: &BlockId<Block>) -> Result<Block::Hash> {
		self.block_hash_from_id(id)
			.and_then(|n| n.ok_or_else(|| Error::UnknownBlock(format!("{}", id))))
	}
}

/// Blockchain database backend. Does not perform any validation.
pub trait Backend<Block: BlockT>: HeaderBackend<Block> {
	/// Get block body. Returns `None` if block is not found.
	fn body(&self, id: BlockId<Block>) -> Result<Option<Vec<<Block as BlockT>::Extrinsic>>>;
	/// Get block justification. Returns `None` if justification does not exist.
	fn justification(&self, id: BlockId<Block>) -> Result<Option<Justification>>;
	/// Get last finalized block hash.
	fn last_finalized(&self) -> Result<Block::Hash>;
	/// Returns data cache reference, if it is enabled on this backend.
	fn cache(&self) -> Option<Arc<dyn Cache<Block>>>;

	/// Returns hashes of all blocks that are leaves of the block tree.
	/// in other words, that have no children, are chain heads.
	/// Results must be ordered best (longest, highest) chain first.
	fn leaves(&self) -> Result<Vec<Block::Hash>>;

	/// Return hashes of all blocks that are children of the block with `parent_hash`.
	fn children(&self, parent_hash: Block::Hash) -> Result<Vec<Block::Hash>>;
}

/// Provides access to the optional cache.
pub trait ProvideCache<Block: BlockT> {
	/// Returns data cache reference, if it is enabled on this backend.
	fn cache(&self) -> Option<Arc<dyn Cache<Block>>>;
}

/// Blockchain optional data cache.
pub trait Cache<Block: BlockT>: Send + Sync {
	/// Initialize genesis value for the given cache.
	///
	/// The operation should be performed once before anything else is inserted in the cache.
	/// Otherwise cache may end up in inconsistent state.
	fn initialize(&self, key: &well_known_cache_keys::Id, value_at_genesis: Vec<u8>) -> Result<()>;
	/// Returns cached value by the given key.
	///
	/// Returned tuple is the range where value has been active and the value itself.
	fn get_at(
		&self,
		key: &well_known_cache_keys::Id,
		block: &BlockId<Block>,
	) -> Option<((NumberFor<Block>, Block::Hash), Option<(NumberFor<Block>, Block::Hash)>, Vec<u8>)>;
}

/// Blockchain info
#[derive(Debug)]
pub struct Info<Block: BlockT> {
	/// Best block hash.
	pub best_hash: Block::Hash,
	/// Best block number.
	pub best_number: <<Block as BlockT>::Header as HeaderT>::Number,
	/// Genesis block hash.
	pub genesis_hash: Block::Hash,
	/// The head of the finalized chain.
	pub finalized_hash: Block::Hash,
	/// Last finalized block number.
	pub finalized_number: <<Block as BlockT>::Header as HeaderT>::Number,
}

/// Block status.
#[derive(Debug, PartialEq, Eq)]
pub enum BlockStatus {
	/// Already in the blockchain.
	InChain,
	/// Not in the queue or the blockchain.
	Unknown,
}

/// An entry in a tree route.
#[derive(Debug)]
pub struct RouteEntry<Block: BlockT> {
	/// The number of the block.
	pub number: <Block::Header as HeaderT>::Number,
	/// The hash of the block.
	pub hash: Block::Hash,
}

/// A tree-route from one block to another in the chain.
///
/// All blocks prior to the pivot in the deque is the reverse-order unique ancestry
/// of the first block, the block at the pivot index is the common ancestor,
/// and all blocks after the pivot is the ancestry of the second block, in
/// order.
///
/// The ancestry sets will include the given blocks, and thus the tree-route is
/// never empty.
///
/// ```text
/// Tree route from R1 to E2. Retracted is [R1, R2, R3], Common is C, enacted [E1, E2]
///   <- R3 <- R2 <- R1
///  /
/// C
///  \-> E1 -> E2
/// ```
///
/// ```text
/// Tree route from C to E2. Retracted empty. Common is C, enacted [E1, E2]
/// C -> E1 -> E2
/// ```
#[derive(Debug)]
pub struct TreeRoute<Block: BlockT> {
	route: Vec<RouteEntry<Block>>,
	pivot: usize,
}

impl<Block: BlockT> TreeRoute<Block> {
	/// Get a slice of all retracted blocks in reverse order (towards common ancestor)
	pub fn retracted(&self) -> &[RouteEntry<Block>] {
		&self.route[..self.pivot]
	}

	/// Get the common ancestor block. This might be one of the two blocks of the
	/// route.
	pub fn common_block(&self) -> &RouteEntry<Block> {
		self.route.get(self.pivot).expect("tree-routes are computed between blocks; \
			which are included in the route; \
			thus it is never empty; qed")
	}

	/// Get a slice of enacted blocks (descendents of the common ancestor)
	pub fn enacted(&self) -> &[RouteEntry<Block>] {
		&self.route[self.pivot + 1 ..]
	}
}

/// Compute a tree-route between two blocks. See tree-route docs for more details.
pub fn tree_route<Block: BlockT, F: Fn(BlockId<Block>) -> Result<<Block as BlockT>::Header>>(
	load_header: F,
	from: BlockId<Block>,
	to: BlockId<Block>,
) -> Result<TreeRoute<Block>> {
	let mut from = load_header(from)?;
	let mut to = load_header(to)?;

	let mut from_branch = Vec::new();
	let mut to_branch = Vec::new();

	while to.number() > from.number() {
		to_branch.push(RouteEntry {
			number: to.number().clone(),
			hash: to.hash(),
		});

		to = load_header(BlockId::Hash(*to.parent_hash()))?;
	}

	while from.number() > to.number() {
		from_branch.push(RouteEntry {
			number: from.number().clone(),
			hash: from.hash(),
		});
		from = load_header(BlockId::Hash(*from.parent_hash()))?;
	}

	// numbers are equal now. walk backwards until the block is the same

	while to != from {
		to_branch.push(RouteEntry {
			number: to.number().clone(),
			hash: to.hash(),
		});
		to = load_header(BlockId::Hash(*to.parent_hash()))?;

		from_branch.push(RouteEntry {
			number: from.number().clone(),
			hash: from.hash(),
		});
		from = load_header(BlockId::Hash(*from.parent_hash()))?;
	}

	// add the pivot block. and append the reversed to-branch (note that it's reverse order originalls)
	let pivot = from_branch.len();
	from_branch.push(RouteEntry {
		number: to.number().clone(),
		hash: to.hash(),
	});
	from_branch.extend(to_branch.into_iter().rev());

	Ok(TreeRoute {
		route: from_branch,
		pivot,
	})
}

/// A list of all well known keys in the blockchain cache.
pub mod well_known_cache_keys {
	/// The type representing cache keys.
	pub type Id = consensus::import_queue::CacheKeyId;

	/// A list of authorities.
	pub const AUTHORITIES: Id = *b"auth";

	/// Current Epoch data.
	pub const EPOCH: Id = *b"epch";

	/// Changes trie configuration.
	pub const CHANGES_TRIE_CONFIG: Id = *b"chtr";
}
