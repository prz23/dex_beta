// Copyright 2019 Parity Technologies (UK) Ltd.
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

//! Mock file for phragmen.

#![cfg(test)]

use crate::{elect, ACCURACY, PhragmenResult};
use sr_primitives::traits::{Convert, Member, SaturatedConversion};
use rstd::collections::btree_map::BTreeMap;
use support::assert_eq_error_rate;

pub(crate) struct TestCurrencyToVote;
impl Convert<Balance, u64> for TestCurrencyToVote {
	fn convert(x: Balance) -> u64 { x.saturated_into() }
}
impl Convert<u128, Balance> for TestCurrencyToVote {
	fn convert(x: u128) -> Balance { x }
}

#[derive(Default, Debug)]
pub(crate) struct _Candidate<A> {
	who: A,
	score: f64,
	approval_stake: f64,
	elected: bool,
}

#[derive(Default, Debug)]
pub(crate) struct _Voter<A> {
	who: A,
	edges: Vec<_Edge<A>>,
	budget: f64,
	load: f64,
}

#[derive(Default, Debug)]
pub(crate) struct _Edge<A> {
	who: A,
	load: f64,
	candidate_index: usize,
}

#[derive(Default, Debug, PartialEq)]
pub(crate) struct _Support<A> {
	pub own: f64,
	pub total: f64,
	pub others: Vec<_PhragmenAssignment<A>>,
}

pub(crate) type _PhragmenAssignment<A> = (A, f64);
pub(crate) type _SupportMap<A> = BTreeMap<A, _Support<A>>;

pub(crate) type Balance = u128;
pub(crate) type AccountId = u64;

#[derive(Debug, Clone)]
pub(crate) struct _PhragmenResult<A: Clone> {
	pub winners: Vec<(A, Balance)>,
	pub assignments: Vec<(A, Vec<_PhragmenAssignment<A>>)>
}

pub(crate) fn elect_float<A, FS>(
	candidate_count: usize,
	minimum_candidate_count: usize,
	initial_candidates: Vec<A>,
	initial_voters: Vec<(A, Vec<A>)>,
	stake_of: FS,
	self_vote: bool,
) -> Option<_PhragmenResult<A>> where
	A: Default + Ord + Member + Copy,
	for<'r> FS: Fn(&'r A) -> Balance,
{
	let mut elected_candidates: Vec<(A, Balance)>;
	let mut assigned: Vec<(A, Vec<_PhragmenAssignment<A>>)>;
	let mut c_idx_cache = BTreeMap::<A, usize>::new();
	let num_voters = initial_candidates.len() + initial_voters.len();
	let mut voters: Vec<_Voter<A>> = Vec::with_capacity(num_voters);

	let mut candidates = if self_vote {
		initial_candidates.into_iter().map(|who| {
			let stake = stake_of(&who) as f64;
			_Candidate { who, approval_stake: stake, ..Default::default() }
		})
		.filter(|c| c.approval_stake != 0f64)
		.enumerate()
		.map(|(i, c)| {
			let who = c.who;
			voters.push(_Voter {
				who: who.clone(),
				edges: vec![
					_Edge { who: who.clone(), candidate_index: i, ..Default::default() }
				],
				budget: c.approval_stake,
				load: 0f64,
			});
			c_idx_cache.insert(c.who.clone(), i);
			c
		})
		.collect::<Vec<_Candidate<A>>>()
	} else {
		initial_candidates.into_iter()
			.enumerate()
			.map(|(idx, who)| {
				c_idx_cache.insert(who.clone(), idx);
				_Candidate { who, ..Default::default() }
			})
			.collect::<Vec<_Candidate<A>>>()
	};

	if candidates.len() < minimum_candidate_count {
		return None;
	}

	voters.extend(initial_voters.into_iter().map(|(who, votes)| {
		let voter_stake = stake_of(&who) as f64;
		let mut edges: Vec<_Edge<A>> = Vec::with_capacity(votes.len());
		for v in votes {
			if let Some(idx) = c_idx_cache.get(&v) {
				candidates[*idx].approval_stake = candidates[*idx].approval_stake + voter_stake;
				edges.push(
					_Edge { who: v.clone(), candidate_index: *idx, ..Default::default() }
				);
			}
		}
		_Voter {
			who,
			edges: edges,
			budget: voter_stake,
			load: 0f64,
		}
	}));

	let to_elect = candidate_count.min(candidates.len());
	elected_candidates = Vec::with_capacity(candidate_count);
	assigned = Vec::with_capacity(candidate_count);

	for _round in 0..to_elect {
		for c in &mut candidates {
			if !c.elected {
				c.score = 1.0 / c.approval_stake;
			}
		}
		for n in &voters {
			for e in &n.edges {
				let c = &mut candidates[e.candidate_index];
				if !c.elected && !(c.approval_stake == 0f64) {
					c.score += n.budget * n.load / c.approval_stake;
				}
			}
		}

		if let Some(winner) = candidates
			.iter_mut()
			.filter(|c| !c.elected)
			.min_by(|x, y| x.score.partial_cmp(&y.score).unwrap_or(rstd::cmp::Ordering::Equal))
		{
			winner.elected = true;
			for n in &mut voters {
				for e in &mut n.edges {
					if e.who == winner.who {
						e.load = winner.score - n.load;
						n.load = winner.score;
					}
				}
			}

			elected_candidates.push((winner.who.clone(), winner.approval_stake as Balance));
		} else {
			break
		}
	}

	for n in &mut voters {
		let mut assignment = (n.who.clone(), vec![]);
		for e in &mut n.edges {
			if let Some(c) = elected_candidates.iter().cloned().map(|(c, _)| c).find(|c| *c == e.who) {
				if c != n.who {
					let ratio = e.load / n.load;
					assignment.1.push((e.who.clone(), ratio));
				}
			}
		}
		if assignment.1.len() > 0 {
			assigned.push(assignment);
		}
	}

	Some(_PhragmenResult {
		winners: elected_candidates,
		assignments: assigned,
	})
}

pub(crate) fn equalize_float<A, FS>(
	mut assignments: Vec<(A, Vec<_PhragmenAssignment<A>>)>,
	supports: &mut _SupportMap<A>,
	tolerance: f64,
	iterations: usize,
	stake_of: FS,
) where
	for<'r> FS: Fn(&'r A) -> Balance,
	A: Ord + Clone + std::fmt::Debug,
{
	for _i in 0..iterations {
		let mut max_diff = 0.0;
		for (voter, assignment) in assignments.iter_mut() {
			let voter_budget = stake_of(&voter);
			let diff = do_equalize_float(
				voter,
				voter_budget,
				assignment,
				supports,
				tolerance,
			);
			if diff > max_diff { max_diff = diff; }
		}

		if max_diff < tolerance {
			break;
		}
	}
}

pub(crate) fn do_equalize_float<A>(
	voter: &A,
	budget_balance: Balance,
	elected_edges: &mut Vec<_PhragmenAssignment<A>>,
	support_map: &mut _SupportMap<A>,
	tolerance: f64
) -> f64 where
	A: Ord + Clone,
{
	let budget = budget_balance as f64;
	if elected_edges.is_empty() { return 0.0; }

	let stake_used = elected_edges
		.iter()
		.fold(0.0, |s, e| s + e.1);

	let backed_stakes_iter = elected_edges
		.iter()
		.filter_map(|e| support_map.get(&e.0))
		.map(|e| e.total);

	let backing_backed_stake = elected_edges
		.iter()
		.filter(|e| e.1 > 0.0)
		.filter_map(|e| support_map.get(&e.0))
		.map(|e| e.total)
		.collect::<Vec<f64>>();

	let mut difference;
	if backing_backed_stake.len() > 0 {
		let max_stake = backing_backed_stake
			.iter()
			.max_by(|x, y| x.partial_cmp(&y).unwrap_or(rstd::cmp::Ordering::Equal))
			.expect("vector with positive length will have a max; qed");
		let min_stake = backed_stakes_iter
			.min_by(|x, y| x.partial_cmp(&y).unwrap_or(rstd::cmp::Ordering::Equal))
			.expect("iterator with positive length will have a min; qed");

		difference = max_stake - min_stake;
		difference = difference + budget - stake_used;
		if difference < tolerance {
			return difference;
		}
	} else {
		difference = budget;
	}

	// Undo updates to support
	elected_edges.iter_mut().for_each(|e| {
		if let Some(support) = support_map.get_mut(&e.0) {
			support.total = support.total - e.1;
			support.others.retain(|i_support| i_support.0 != *voter);
		}
		e.1 = 0.0;
	});

	// todo: rewrite.
	elected_edges.sort_unstable_by(|x, y|
		if let Some(x) = support_map.get(&x.0) {
			if let Some(y) = support_map.get(&y.0) {
				x.total.partial_cmp(&y.total).unwrap_or(rstd::cmp::Ordering::Equal)
			} else {
				rstd::cmp::Ordering::Equal
			}
		} else {
			rstd::cmp::Ordering::Equal
		}
	);

	let mut cumulative_stake = 0.0;
	let mut last_index = elected_edges.len() - 1;
	elected_edges.iter_mut().enumerate().for_each(|(idx, e)| {
		if let Some(support) = support_map.get_mut(&e.0) {
			let stake = support.total;
			let stake_mul = stake * (idx as f64);
			let stake_sub = stake_mul - cumulative_stake;
			if stake_sub > budget {
				last_index = idx.checked_sub(1).unwrap_or(0);
				return
			}
			cumulative_stake = cumulative_stake + stake;
		}
	});

	let last_stake = elected_edges[last_index].1;
	let split_ways = last_index + 1;
	let excess = budget + cumulative_stake - last_stake * (split_ways as f64);
	elected_edges.iter_mut().take(split_ways).for_each(|e| {
		if let Some(support) = support_map.get_mut(&e.0) {
			e.1 = excess / (split_ways as f64) + last_stake - support.total;
			support.total = support.total + e.1;
			support.others.push((voter.clone(), e.1));
		}
	});

	difference
}


pub(crate) fn create_stake_of(stakes: &[(AccountId, Balance)])
	-> Box<dyn Fn(&AccountId) -> Balance>
{
	let mut storage = BTreeMap::<AccountId, Balance>::new();
	stakes.iter().for_each(|s| { storage.insert(s.0, s.1); });
	let stake_of = move |who: &AccountId| -> Balance { storage.get(who).unwrap().to_owned() };
	Box::new(stake_of)
}

pub(crate) fn run_and_compare(
	candidates: Vec<AccountId>,
	voters: Vec<(AccountId, Vec<AccountId>)>,
	stake_of: Box<dyn Fn(&AccountId) -> Balance>,
	to_elect: usize,
	min_to_elect: usize,
	self_vote: bool,
) {
	// run fixed point code.
	let PhragmenResult { winners, assignments } = elect::<_, _, _, TestCurrencyToVote>(
		to_elect,
		min_to_elect,
		candidates.clone(),
		voters.clone(),
		&stake_of,
		self_vote,
	).unwrap();

	// run float poc code.
	let truth_value = elect_float(
		to_elect,
		min_to_elect,
		candidates,
		voters,
		&stake_of,
		self_vote,
	).unwrap();

	assert_eq!(winners, truth_value.winners);

	for (nominator, assigned) in assignments.clone() {
		if let Some(float_assignments) = truth_value.assignments.iter().find(|x| x.0 == nominator) {
			for (candidate, ratio) in assigned {
				if let Some(float_assignment) = float_assignments.1.iter().find(|x| x.0 == candidate ) {
					assert_eq_error_rate!((float_assignment.1 * ACCURACY as f64).round() as u128, ratio, 1);
				} else {
					panic!("candidate mismatch. This should never happen.")
				}
			}
		} else {
			panic!("nominator mismatch. This should never happen.")
		}
	}
}

pub(crate) fn build_support_map<FS>(
	result: &mut _PhragmenResult<AccountId>,
	stake_of: FS,
) -> _SupportMap<AccountId>
	where for<'r> FS: Fn(&'r AccountId) -> Balance
{
	let mut supports = <_SupportMap<AccountId>>::new();
	result.winners
		.iter()
		.map(|(e, _)| (e, stake_of(e) as f64))
		.for_each(|(e, s)| {
			let item = _Support { own: s, total: s, ..Default::default() };
			supports.insert(e.clone(), item);
		});

	for (n, assignment) in result.assignments.iter_mut() {
		for (c, r) in assignment.iter_mut() {
			let nominator_stake = stake_of(n) as f64;
			let other_stake = nominator_stake * *r;
			if let Some(support) = supports.get_mut(c) {
				support.total = support.total + other_stake;
				support.others.push((n.clone(), other_stake));
			}
			*r = other_stake;
		}
	}

	supports
}
