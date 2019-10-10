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

//! # Sudo Module
//!
//! - [`sudo::Trait`](./trait.Trait.html)
//! - [`Call`](./enum.Call.html)
//!
//! ## Overview
//!
//! The Sudo module allows for a single account (called the "sudo key")
//! to execute dispatchable functions that require a `Root` call
//! or designate a new account to replace them as the sudo key.
//! Only one account can be the sudo key at a time.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! Only the sudo key can call the dispatchable functions from the Sudo module.
//!
//! * `sudo` - Make a `Root` call to a dispatchable function.
//! * `set_key` - Assign a new account to be the sudo key.
//!
//! ## Usage
//!
//! ### Executing Privileged Functions
//!
//! The Sudo module itself is not intended to be used within other modules.
//! Instead, you can build "privileged functions" (i.e. functions that require `Root` origin) in other modules.
//! You can execute these privileged functions by calling `sudo` with the sudo key account.
//! Privileged functions cannot be directly executed via an extrinsic.
//!
//! Learn more about privileged functions and `Root` origin in the [`Origin`] type documentation.
//!
//! ### Simple Code Snippet
//!
//! This is an example of a module that exposes a privileged function:
//!
//! ```
//! use support::{decl_module, dispatch::Result};
//! use system::ensure_root;
//!
//! pub trait Trait: system::Trait {}
//!
//! decl_module! {
//!     pub struct Module<T: Trait> for enum Call where origin: T::Origin {
//!         pub fn privileged_function(origin) -> Result {
//!             ensure_root(origin)?;
//!
//!             // do something...
//!
//!             Ok(())
//!         }
//!     }
//! }
//! # fn main() {}
//! ```
//!
//! ## Genesis Config
//!
//! The Sudo module depends on the [`GenesisConfig`](./struct.GenesisConfig.html).
//! You need to set an initial superuser account as the sudo `key`.
//!
//! ## Related Modules
//!
//! * [Consensus](../srml_consensus/index.html)
//! * [Democracy](../srml_democracy/index.html)
//!
//! [`Call`]: ./enum.Call.html
//! [`Trait`]: ./trait.Trait.html
//! [`Origin`]: https://docs.substrate.dev/docs/substrate-types

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
#[macro_use]
extern crate serde_derive;

use rstd::prelude::*;
use sr_primitives::{
	traits::{StaticLookup, Dispatchable}, weights::SimpleDispatchInfo, DispatchError,
};
use support::{StorageValue, StorageMap, Parameter, decl_module, decl_event, decl_storage, ensure,
              dispatch::Result};
use support::storage::{generator};

use rstd::marker::PhantomData;

use balances::*;

use system::ensure_signed;
use codec::{Encode, Decode, Codec};

pub mod linked_node;
use linked_node::*;
use token::Token;


#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct OrderPair {
	pub first: Vec<u8>,
	pub second: Vec<u8>,
}

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub enum OrderType {
	Buy,
	Sell,
}

impl Default for OrderType {
	fn default() -> Self {
		OrderType::Buy
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub enum CancelOrMatch {
	Match,
	Cancel,
}

impl Default for CancelOrMatch {
	fn default() -> Self {
		CancelOrMatch::Match
	}
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct Bid<Amount, Price, A>
	where
		Amount: Copy,
		Price: Copy,
{
	nodeid: u128,
	price: Price,
	sum: Amount,
	list: Vec<u128>,
    useless: A,
}

pub type BidT<T> = Bid<u64, u64, <T as balances::Trait>::Balance>;

impl<Amount, Price, A> NodeT for Bid<Amount, Price, A>
	where
		Price: Codec + Clone + Eq + PartialEq + Default + Copy,
		Amount: Copy,
{
	type Index = u128;

	fn index(&self) -> Self::Index {
		self.nodeid
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub enum OrderStatus {
	Valid,
	Canceled,
	Finished,
}

impl Default for OrderStatus {
	fn default() -> Self {
		OrderStatus::Valid
	}
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct OrderInformation<Who, BlockNumber>{
	who: Who,
	ordertype: OrderType,   // buy or sell
	pair: OrderPair,
	amount: u64,
	price: u64,
	left: u64,              // the leftover of balance in this order
	status: OrderStatus,    //the status of an order
	index: u128,            // an unique index of an order
	time: BlockNumber,      //order creation time
	fill_index: Vec<u128>,  // index of other orders that matched with this order
}

impl <Who, BlockNumber> OrderInformation<Who, BlockNumber>{
	pub fn new(who: Who,
			   ordertype: OrderType,
			   pair: OrderPair,
			   amount: u64,
			   price: u64,
			   index: u128,
	           time: BlockNumber) -> Self {
		OrderInformation {
			who: who,
			ordertype: ordertype,
			pair: pair,
			amount: amount,
			price: price,
			left: amount,
			index: index,
			status: OrderStatus::default(),
			time:time,
			fill_index: Default::default(),
		}
	}

	pub fn money_change(&mut self, money:u64, add:bool) -> bool {
		self.left = self.left - money;
		if self.left == 0 {
			self.status = OrderStatus::Finished;
			return false;
		}
		return true;
	}

	pub fn lock_each_token(){

	}
}

pub type OrderInfo<T> = OrderInformation<<T as system::Trait>::AccountId, <T as system::Trait>::BlockNumber>;


/// 盘口 记录 详情
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct BidDetail<AccountId, BlockNumber>
	where
		AccountId: Clone,
		BlockNumber: Copy,
{
	id: u128,
	pair: OrderPair,
	order_type: OrderType,
	user: AccountId,
	price: u64,
	amount: u64,
	time: BlockNumber,
}

pub type BidDetailT<T> = BidDetail<
	<T as system::Trait>::AccountId,
	<T as system::Trait>::BlockNumber,
>;


pub struct LinkedMultiKey<T: Trait>(PhantomData<T>);
impl<T: Trait> LinkedNodeCollection for LinkedMultiKey<T> {
	type Header = BidListHeaderFor<T>;
	type NodeMap = BidListCache<T>;
	type Tail = BidListTailFor<T>;
}

pub trait Trait: system::Trait + balances::Trait + token::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
	// Simple declaration of the `Module` type. Lets the macro know what it's working on.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

	    fn put_order_and_match(origin, orderpair:OrderPair, ordertype:OrderType, amount:u64, price:u64) -> Result{
		    let sender = ensure_signed(origin)?;
		    Self::check_order(sender,orderpair,ordertype,amount,price)?;

		    Ok(())
		}

		fn cancel_order(origin, orderpair:OrderPair, index:u128) -> Result {
		    let sender = ensure_signed(origin)?;
		    Self::do_cancel_order(&sender,orderpair,index)?;
            Ok(())
		}

		fn on_finalize() {
            // handle the match of new orders
		}

	}
}

decl_event!(
	pub enum Event<T> where AccountId = <T as system::Trait>::AccountId {
		KeyChanged(AccountId),
		NewOrder(AccountId,u128),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as Dex {
		/// The `AccountId` of the sudo key.
		Key get(key) config(): T::AccountId;
        //BP get(bp) config(): OrderPair;
		/// each order have an uinque u128 index
		pub OrderIndex get(order_index): u128 = 0;

        /// Order pair list
        pub OrderPairList get(order_pair_list): Vec<OrderPair> ;

		/// save the data
        pub BidListHeaderFor get(bidlist_header_for): map (OrderPair,OrderType) => Option<MultiNodeIndex<(OrderPair,OrderType), BidT<T>>>;
        pub BidListTailFor get(bidlist_tail_for): map (OrderPair,OrderType) => Option<MultiNodeIndex<(OrderPair,OrderType), BidT<T>>>;
        pub BidListCache get(bidlist_cache): map u128 => Option<Node<BidT<T>>>;
        NodeId get(nodeid):u128;
        pub BidOf get(bid_of):map u128 => Option<BidDetailT<T>>;

        // real order record
        pub OrderInfor get(order_info): map u128 => Option<OrderInfo<T>>;
	}
		add_extra_genesis {
		    config(bp):  u64;
            build(|config: &GenesisConfig<T>|  {
                OrderPairList::put(vec![OrderPair{
                    first: vec![1u8,2],
                    second: vec![3u8,4],
                }]);
			});
		}
}

impl<T: Trait> Module<T> {
	pub fn check_order(
		who: T::AccountId,
		pair: OrderPair,
		ordertype: OrderType,
		amount: u64,
		price: u64,) -> Result{

		// check the validity of new order
        ensure!( price != 0u64 , "price can not be 0.");
		ensure!( amount != 0u64 , "amount can not be 0.");
		Self::is_vaild_pair(&pair)?;
		Self::enough_token_and_lock(&who,ordertype.clone(),&pair,price,amount)?;

		// save the new order and deposit new event
		let mut order = Self::save_new_order(|unique_index|{ OrderInformation::new(who.clone(), ordertype,
																				   pair.clone(), amount, price,
																				   unique_index as u128,
																        <system::Module<T>>::block_number(), ) });
        // match order and change the status of old order and modify tokens
		Self::insert_and_match_order(order.ordertype.clone(),&mut order);
		Ok(())
	}

	pub fn enough_token_and_lock(who:&T::AccountId, ordertype:OrderType,
								 pair:&OrderPair, price:u64, amount:u64) -> Result {
		//T::Token::lock   <T as Token::Trait>::lock <token::Module<T>>::
		<token::Module<T>>::lock(&who,(||{match ordertype {
			OrderType::Buy => &pair.second,
			OrderType::Sell => &pair.first, }})(),
					   (||{match ordertype {
						   OrderType::Buy => price*amount,
						   OrderType::Sell => amount, }})()
		)?;
		Ok(())
	}

	pub fn save_new_order<F>(mut func: F) -> OrderInfo<T>
		where F: FnMut(u128) -> OrderInfo<T>
	{
		let unique_index = OrderIndex::get() + 1;
		let order : OrderInfo<T> = func(unique_index);
		<OrderInfor<T>>::insert(unique_index.clone(),&order);
		OrderIndex::put(unique_index);
		Self::deposit_event(RawEvent::NewOrder(order.who.clone(),unique_index));
		order
	}

	pub fn insert_and_match_order(find_type: OrderType, order_info: &mut OrderInfo<T>){
        // change the order information into biddetail
		let mut new_biddetail = BidDetail{
			id: order_info.index.clone(),
			pair: order_info.pair.clone(),
			order_type: order_info.ordertype.clone(),
			user: order_info.who.clone(),
			price: order_info.price.clone(),
			amount: order_info.amount.clone(),
			time: order_info.time.clone() ,
		};

		let find_type: OrderType = match new_biddetail.order_type {
			OrderType::Buy => OrderType::Sell,
			OrderType::Sell => OrderType::Buy,
		};

		Self::do_match(find_type, &mut new_biddetail);

		if new_biddetail.amount == 0 {
			//已被匹配完毕，则删除
			<BidOf<T>>::remove(new_biddetail.id);
		} else {
			//如果还有剩余，则将其更新到bid_list中
			//println!("如果还有剩余，则将其更新到bid_list中");
			Self::insert_bid_list(&new_biddetail);
		}
	}

	fn do_match(find_type: OrderType, in_bid_detail: &mut BidDetailT<T>) {
		let mut need_fill: u64 = in_bid_detail.amount;
		let mut remove_from_wait_bid_list: Vec<BidT<T>> = Vec::new();

		if let Some(header) = Self::bidlist_header_for((in_bid_detail.pair.clone(), find_type)) {
			let mut index = header.index();

			while let Some(mut node) = Self::bidlist_cache(&index) {
				let mut find_match = false;

				match in_bid_detail.order_type {
					OrderType::Sell => {
						if (need_fill != 0) && (in_bid_detail.price <= node.data.price) {
							find_match = true;
						}
					}
					OrderType::Buy => {
						if (need_fill != 0) && (in_bid_detail.price >= node.data.price) {
							find_match = true;
						}
					}
				}
////////////////////////
				if find_match == true {
					// match and change token
					let mut fill_num: u64;
					if need_fill < node.data.sum {
						fill_num = need_fill;
					} else {
						fill_num = node.data.sum;
						remove_from_wait_bid_list.push(node.data.clone());
					}
					need_fill = need_fill - fill_num;
					in_bid_detail.amount = in_bid_detail.amount - fill_num;
					node.data.sum = node.data.sum - fill_num;
					// match each bid in this node and record the matched bids
					let mut remove_from_list: Vec<u128> = Vec::new();

					for kk in 0..node.data.list.len() {
						if let Some(mut match_bid) = Self::bid_of(node.data.list[kk]) {
							let maker_user = match_bid.user.clone();
							let taker_user = in_bid_detail.user.clone();
							let maker_user_order_index = match_bid.id;
							let taker_user_order_index = in_bid_detail.id;
							let order_price = match_bid.price;
							let mut amount: u64;


							if fill_num >= match_bid.amount {
								amount = match_bid.amount;
								// remove matched bid
								<BidOf<T>>::remove(match_bid.id);
								remove_from_list.push(match_bid.id);
							} else {
								amount = fill_num;
								match_bid.amount = match_bid.amount - amount;
								<BidOf<T>>::insert(match_bid.id, match_bid.clone());
							}

							fill_num = fill_num - amount;

							//let maker_fee: u64 = Self::calculation_maker_fee(amount); // 手续费
							//let taker_fee: u64 = Self::calculation_taker_fee(amount); // 手续费

							let mut aaaaa = T::AccountId::default();
							let mut bbbbb = T::AccountId::default();
							match in_bid_detail.order_type{
								OrderType::Buy => { aaaaa = maker_user; bbbbb = taker_user; }, // maker user = matchbid
								OrderType::Sell =>{ aaaaa = taker_user; bbbbb = maker_user; }, // taker user = in_bid_detail
							}
							<token::Module<T>>::exchange_token(&aaaaa,&bbbbb,
															   in_bid_detail.pair.first.clone(),
															   in_bid_detail.pair.second.clone(),amount,match_bid.price,
							                                   in_bid_detail.price);

							Self::modify_order_and_generate_the_deal_record(match_bid.id,in_bid_detail.id,
							                     amount,match_bid.price);

							if fill_num == 0 {
								break;
							}
						}
					}

					Self::remove_from_bid_list(&mut node, &remove_from_list);

					if let Some(next) = node.next() {
						index = next;
					} else {
						break;
					}
				} else {
					break;
				}
			}
		}
////////////////////////////////////////////////////////////
		// remove full matched bids
		Self::remove_from_bid(&in_bid_detail.pair, find_type, &remove_from_wait_bid_list);
	}

	pub fn do_cancel_order(who:&T::AccountId, order:OrderPair, index:u128) -> Result{

		if let Some(mut order) = Self::order_info(index) {
			if order.who != *who{ return Err("not permitted");}
            match order.status {
				OrderStatus::Valid => {
					order.status = OrderStatus::Canceled;
					// unlock token
					let tokentype = match order.ordertype {
						OrderType::Buy => order.pair.second.clone(),
						OrderType::Sell => order.pair.first.clone() ,
					};
					<token::Module<T>>::unlock(&order.who,&tokentype,order.left);

				},
				_ => return Err("Canceled or Finished"),
			}
			<OrderInfor<T>>::insert(order.index,order);
		}else { return Err("cant find order"); }
		Ok(())
	}

	pub fn is_vaild_pair(orderpair:&OrderPair) -> Result{
		Ok(())
	}

	pub fn token_exchange(){
		//<token::Module<T>>::exchange_token();
		//<Self as Token<_>>::transfer(&sender,&dest,&tokentype,value)?;
	}



////////////////////////////////////////////////////////////////////////////////////////////////////
	fn insert_bid_list(in_bid_detail: &BidDetailT<T>) {
		<BidOf<T>>::insert(in_bid_detail.id, in_bid_detail.clone());

		let mut finish = false;
		if let Some(header) =
		Self::bidlist_header_for((in_bid_detail.pair.clone(), in_bid_detail.order_type))
		{
			let mut index = header.index();

			while let Some(mut node) = Self::bidlist_cache(&index) {
				// 从该类型的head（头节点）开始，首先找到价格相等的地方
				//   1. 如果找到了，就把当前的bid数量加入节点总量，并把自己index放入节点的list中
				// 然后把节点存回去，同时跳出循环，流程全部结束！
				//   2. 如果没有，查看下是否需要自己当新的节点插入当前的节点前面，是的话就insert在当前节点的head
				//  => 如果1和2都不满足，那么就是一个新价格的节点插到tail的地方
				if in_bid_detail.price == node.data.price {
					//累加
					node.data.sum += in_bid_detail.amount;
					node.data.list.push(in_bid_detail.id);

					<BidListCache<T>>::insert(node.index(), node);

					finish = true;
					break;
				}
				let mut insert_head = false;

				match in_bid_detail.order_type {
					OrderType::Sell => {
						if in_bid_detail.price < node.data.price {
							//插入当前的 前面
							insert_head = true;
						}
					}
					OrderType::Buy => {
						if in_bid_detail.price > node.data.price {
							insert_head = true;
						}
					}
				}

				if insert_head == true {
					let new_nodeid = Self::new_nodeid();
					let mut list_vec: Vec<u128> = Vec::new();
					list_vec.push(in_bid_detail.id);

					let new_bid = Bid {
						nodeid: new_nodeid,
						price: in_bid_detail.price,
						sum: in_bid_detail.amount,
						list: list_vec,
						useless: <T as balances::Trait>::Balance::from(0),
					};

					let n = Node::new(new_bid);
					n.init_storage_withkey::<LinkedMultiKey<T>, (OrderPair, OrderType)>((
						in_bid_detail.pair.clone(),
						in_bid_detail.order_type,
					));

					let _=node.add_option_node_before_withkey::<LinkedMultiKey<T>, (OrderPair,OrderType)>(n,(in_bid_detail.pair.clone(),in_bid_detail.order_type));

					finish = true;
					break;
				}

				if let Some(next) = node.next() {
					index = next;
				} else {
					break;
				}
			}
		}
		if finish == false {
			//追加在最后
			let new_nodeid = Self::new_nodeid();
			let mut list_vec: Vec<u128> = Vec::new();
			list_vec.push(in_bid_detail.id);

			let new_bid = Bid {
				nodeid: new_nodeid,
				price: in_bid_detail.price,
				sum: in_bid_detail.amount,
				list: list_vec,
				useless: <T as balances::Trait>::Balance::from(0),
			};
			let n = Node::new(new_bid);
			n.init_storage_withkey::<LinkedMultiKey<T>, (OrderPair, OrderType)>((
				in_bid_detail.pair.clone(),
				in_bid_detail.order_type,
			));

			if let Some(tail_index) =
			Self::bidlist_tail_for((in_bid_detail.pair.clone(), in_bid_detail.order_type))
			{
				if let Some(mut tail_node) = Self::bidlist_cache(tail_index.index()) {
					let _ = tail_node
						.add_option_node_after_withkey::<LinkedMultiKey<T>, (OrderPair, OrderType)>(
							n,
							(in_bid_detail.pair.clone(), in_bid_detail.order_type),
						);
				}
			}
		}
	}

	fn new_nodeid() -> u128 {
		let mut last_nodeid: u128 = <NodeId>::get();
		last_nodeid = match last_nodeid.checked_add(1_u128) {
			Some(b) => b,
			None => 0,
		};
		<NodeId>::put(last_nodeid);

		last_nodeid
	}

	fn remove_from_bid_list(
		node: &mut Node<
			BidT<T>,
		>,
		remove_id: &Vec<u128>,
	) {
		let mut new_list: Vec<u128> = Vec::new();
		for mm in 0..node.data.list.len() {
			let mut remove = false;
			for nn in 0..remove_id.len() {
				if node.data.list[mm] == remove_id[nn] {
					remove = true;
					<BidOf<T>>::remove(remove_id[nn]);
				}
			}
			if remove == false {
				new_list.push(node.data.list[mm]);
			}
		}

		node.data.list = new_list;
		<BidListCache<T>>::insert(node.index(), node);
		//更新node
	}

	fn remove_from_bid(pair: &OrderPair, order_type: OrderType, remove_bid: &Vec<BidT<T>>) {
		for nn in 0..remove_bid.len() {
			if let Some(header) = Self::bidlist_header_for((pair.clone(), order_type)) {
				let mut index = header.index();

				while let Some(mut node) = Self::bidlist_cache(&index) {
					if node.data.price == remove_bid[nn].price {
						let _=node.remove_option_node_withkey::<LinkedMultiKey<T>, (OrderPair,OrderType)>((pair.clone(),order_type));
						break;
					}

					if let Some(next) = node.next() {
						index = next;
					} else {
						break;
					}
				}
			}
		}
	}

	fn cancel_bid(in_bid_detail: &BidDetailT<T>) {
		<BidOf<T>>::remove(in_bid_detail.id);

		let mut remove_from_wait_bid_list: Vec<BidT<T>> = Vec::new();
		if let Some(header) =
		Self::bidlist_header_for((in_bid_detail.pair.clone(), in_bid_detail.order_type))
		{
			let mut index = header.index();

			while let Some(mut node) = Self::bidlist_cache(&index) {
				if node.data.price == in_bid_detail.price {
					node.data.sum = node.data.sum - in_bid_detail.amount;
					if node.data.sum == 0 {
						remove_from_wait_bid_list.push(node.data.clone()); //标记删除
					}
					for mm in 0..node.data.list.len() {
						if in_bid_detail.id == node.data.list[mm] {
							let mut list_vec: Vec<u128> = Vec::new();
							list_vec.push(in_bid_detail.id);

							Self::remove_from_bid_list(&mut node, &list_vec);
							break;
						}
					}
					break;
				}
				if let Some(next) = node.next() {
					index = next;
				} else {
					break;
				}
			}
		}

		Self::remove_from_bid(
			&in_bid_detail.pair,
			in_bid_detail.order_type,
			&remove_from_wait_bid_list,
		); //最后更新
	}

	// add new orderpair
	pub fn add_new_order_pair(pair: OrderPair) -> Result {
		let mut pair_list: Vec<OrderPair> = OrderPairList::get();
		if pair_list.contains(&pair) {
			return Err("already exist orderpair");
		} else {
			pair_list.push(pair);
			OrderPairList::put(pair_list);
			Ok(())
		}
	}

	pub fn modify_order_and_generate_the_deal_record(index_a:u128, index_b:u128, amount:u64, price:u64) -> Result {
		let mut order_a = if let Some(mut order_a) =
		Self::order_info(index_a)
		{
			order_a.left = order_a.left.checked_sub(amount).unwrap();
			order_a.fill_index.push(index_b);
			if order_a.left == 0u64 { order_a.status = OrderStatus::Finished; }
			order_a
		} else {
			return Err("cann't find this maker order");
		};
		<OrderInfor<T>>::insert(order_a.index,order_a);

		let mut order_b = if let Some(mut order_b) =
		Self::order_info(index_b)
		{
			order_b.left = order_b.left.checked_sub(amount).unwrap();
			order_b.fill_index.push(index_a);
			if order_b.left == 0u64 { order_b.status = OrderStatus::Finished; }
			order_b
		} else {
			return Err("cann't find this maker order");
		};
		<OrderInfor<T>>::insert(order_b.index,order_b);

		Ok(())
	}
}
