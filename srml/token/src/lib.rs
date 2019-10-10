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

use rstd::prelude::*;
use sr_primitives::{
	traits::{StaticLookup, Dispatchable, SimpleArithmetic}, weights::SimpleDispatchInfo, DispatchError,
};
use support::{StorageValue, StorageMap, Parameter, decl_module, decl_event, decl_storage, ensure,
             dispatch::Result};
use support::traits::{Currency,ExistenceRequirement,WithdrawReason};
use system::ensure_signed;
use generic_asset;
use codec::{Encode, Decode, Codec};

#[cfg(feature = "std")]
#[macro_use]
extern crate serde_derive;

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub enum TokenControl {
	Free,
	Lock,
}

pub trait Token<AccountId>{
	type Tokens: Copy + Default + SimpleArithmetic;

	fn amount_free(who:&AccountId, tokentype:&Vec<u8>) -> Self::Tokens;

	fn amount_lock(who:&AccountId, tokentype:&Vec<u8>) -> Self::Tokens;

	fn set_free_token(who:&AccountId, tokentype:&Vec<u8>, val:Self::Tokens);

	fn set_lock_token(who:&AccountId, tokentype:&Vec<u8>, val:Self::Tokens);

	fn transfer(source: &AccountId, dest: &AccountId, tokentype:&Vec<u8>, value: Self::Tokens, ) -> Result;

	fn lock(who:&AccountId, tokentype:&Vec<u8>, amount:Self::Tokens) -> Result;

	fn unlock(who:&AccountId, tokentype:&Vec<u8>, amount:Self::Tokens) -> Result;

	fn mint(dest:&AccountId, tokentype:&Vec<u8>, amount:Self::Tokens) -> Result;

	fn burn(dest:&AccountId, tokentype:&Vec<u8>, amount:Self::Tokens) -> Result;

	fn vaild_tokentype(tokentype:&Vec<u8>) -> Result;
}

impl<T:Trait> Token<T::AccountId> for Module<T>{
	type Tokens = u64;

	fn amount_free(who:&T::AccountId, tokentype:&Vec<u8>) -> Self::Tokens{
        <FreeToken<T>>::get((tokentype.clone(),who.clone()))
	}

	fn amount_lock(who:&T::AccountId, tokentype:&Vec<u8>) -> Self::Tokens{
		<LockedToken<T>>::get((tokentype.clone(),who.clone()))
	}

	fn set_free_token(who:&T::AccountId, tokentype:&Vec<u8>, val:Self::Tokens){
		<FreeToken<T>>::insert((tokentype.clone(),who.clone()),val);
	}

	fn set_lock_token(who:&T::AccountId, tokentype:&Vec<u8>, val:Self::Tokens){
		<LockedToken<T>>::insert((tokentype.clone(),who.clone()),val);
	}

	fn transfer(
		source: &T::AccountId,
		dest: &T::AccountId,
		tokentype:&Vec<u8>,
		value: Self::Tokens,
	) -> Result{
		Self::vaild_tokentype(tokentype)?;

		if value > Self::amount_free(source, tokentype){
			return Err("Insufficient available balance");
		}
		let source_new = Self::amount_free(source, tokentype) - value;
		let dest_new = Self::amount_free(dest, tokentype) + value;

		Self::set_free_token(source,tokentype,source_new);
		Self::set_free_token(dest,tokentype,dest_new);

        Ok(())
	}

	fn lock(who:&T::AccountId, tokentype:&Vec<u8>, value:Self::Tokens) -> Result{
		Self::vaild_tokentype(tokentype)?;

		if value > Self::amount_free(who, tokentype){
			return Err("Insufficient available balance");
		}
		let free_new = Self::amount_free(who, tokentype) - value;
		let lcok_new = Self::amount_lock(who, tokentype) + value;

		Self::set_free_token(who,tokentype,free_new);
		Self::set_lock_token(who,tokentype,lcok_new);
		Ok(())
	}

	fn unlock(who:&T::AccountId, tokentype:&Vec<u8>, value:Self::Tokens) -> Result{
		Self::vaild_tokentype(tokentype)?;

		if value > Self::amount_lock(who, tokentype){
			return Err("Insufficient available balance");
		}
		let free_new = Self::amount_free(who, tokentype) + value;
		let lcok_new = Self::amount_lock(who, tokentype) - value;

		Self::set_free_token(who,tokentype,free_new);
		Self::set_lock_token(who,tokentype,lcok_new);
		Ok(())
	}

	fn mint(dest:&T::AccountId, tokentype:&Vec<u8>, value:Self::Tokens) -> Result{
		Self::vaild_tokentype(tokentype)?;

		Self::set_free_token(dest,tokentype,value);
		Ok(())
	}

	fn burn(dest:&T::AccountId, tokentype:&Vec<u8>, value:Self::Tokens) -> Result{
		Self::vaild_tokentype(tokentype)?;

		if value > Self::amount_free(dest, tokentype){
			return Err("Insufficient available balance");
		}
		let new_value = Self::amount_free(dest, tokentype) - value;
		Self::set_free_token(dest,tokentype,new_value);
		Ok(())
	}

	fn vaild_tokentype(tokentype:&Vec<u8>) -> Result{
		if TokenTypeAndPrecision::get(tokentype) == None{
			return Err("invalid tokentype");
		}
		Ok(())
	}
}

// + generic_asset::Trait
pub trait Trait: system::Trait{
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	type Token: Token<<Self as system::Trait>::AccountId>;
}

decl_module! {
	// Simple declaration of the `Module` type. Lets the macro know what it's working on.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

        pub fn transfer_free_token(origin, dest:T::AccountId, tokentype:Vec<u8>, value:u64) -> Result{
            let sender = ensure_signed(origin)?;
            //T::Token::transfer(&sender,&dest,&tokentype,value)?;
            Self::transfer(&sender,&dest,&tokentype,value)?;
            //<Self as Token<_>>::transfer(&sender,&dest,&tokentype,value)?;
            Ok(())
        }

        pub fn add_new_tokentype(origin, tokentypt:Vec<u8>, precision:u64){
            let sender = ensure_signed(origin)?;
            TokenTypeAndPrecision::insert(tokentypt,precision);
        }
	}
}

decl_event!(
	pub enum Event<T> where AccountId = <T as system::Trait>::AccountId {
		/// A sudo just took place.
		Sudid(bool),
		/// The sudoer just switched identity; the old key is supplied.
		KeyChanged(AccountId),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as Token {
		/// The `AccountId` of the sudo key.
		Key get(key) config(): T::AccountId;

        TokenTypeAndPrecision get(token_type_and_precision): map Vec<u8> => Option<u64>;

		FreeToken get(free_token) : map (Vec<u8>,T::AccountId) => u64;
		LockedToken get(locked_token) : map (Vec<u8>,T::AccountId) => u64;
	}
	add_extra_genesis {
		config(bt):  u64;
        build(|config: &GenesisConfig<T>|  {
         <Module<T>>::init_token();
        });
	}
}

impl<T: Trait> Module<T>{

    pub fn init_token(){
		TokenTypeAndPrecision::insert(vec![1u8,2],1000);
		TokenTypeAndPrecision::insert(vec![3u8,4],1000);
	}

	pub fn depositing_token(dest:&T::AccountId, tokentype:Vec<u8>, value:u64) -> Result{
		Self::mint(dest,&tokentype,value)?;
		Ok(())
	}

	pub fn delete_token(dest:&T::AccountId, tokentype:Vec<u8>, value:u64) -> Result{
        Self::burn(dest,&tokentype,value)?;
		Ok(())
	}

	pub fn token_increase_or_decrease(who:&T::AccountId, tokentype:&Vec<u8>, value:u64,
									  changetype:TokenControl ,add:bool)
	{
		let mut new = 0u64;
		match changetype {
			TokenControl::Free => {
				let old = Self::amount_free(who,&tokentype);
				if add { new = old.checked_add(value).unwrap();} else { new = old.checked_sub(value).unwrap(); }
				Self::set_free_token(who,&tokentype,new);},
			TokenControl::Lock => {
				let old = Self::amount_lock(who,&tokentype);
				if add { new = old.checked_add(value).unwrap();} else { new = old.checked_sub(value).unwrap(); }
				Self::set_lock_token(who,&tokentype,new);},
		}
	}



	// after the transcation , exchange the token
	pub fn exchange_token(seller:&T::AccountId, buyer:&T::AccountId, tokentype_share:Vec<u8>,
						  tokentype_money:Vec<u8>, amount:u64, price:u64, lock_price:u64){
/*
		println!("seller={:?} buyer={:?} tokentype_share={:?} tokentype_money={:?} amount={:?} price={:?} lock_price={:?}",
				 seller,buyer,
				 tokentype_share,
				 tokentype_money,amount,price,lock_price);
*/
		let mut extra_lock_to_free = 0u64;
		if lock_price > price {
			extra_lock_to_free = amount * (lock_price - price);
			Self::unlock(buyer,&tokentype_money,extra_lock_to_free);
			//println!("extra_lock_to_free = {:?}",extra_lock_to_free);
		}

		let money = amount * price;
		//println!("test1 {:?} ", Self::amount_lock(buyer,&tokentype_money));
		//money exchange
		Self::token_increase_or_decrease(buyer,&tokentype_money,money,TokenControl::Lock,false);
		Self::token_increase_or_decrease(seller,&tokentype_money,money,TokenControl::Free,true);
		// share exchange
		Self::token_increase_or_decrease(seller,&tokentype_share,amount,TokenControl::Lock,false);
		Self::token_increase_or_decrease(buyer,&tokentype_share,amount,TokenControl::Free,true);
	}

}