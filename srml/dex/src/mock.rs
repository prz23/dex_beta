use runtime_io;
use support::{impl_outer_origin, parameter_types,};
use sr_primitives::Perbill;
use primitives::{H256, Blake2Hasher};
use sr_primitives::traits::{IdentityLookup, Convert, OpaqueKeys, OnInitialize, SaturatedConversion};
use sr_primitives::testing::{Header, UintAuthorityId};
use codec::{Decode,Encode};
use crate::linked_node::*;
use crate::Trait;

/// The AccountId alias in this test module.
pub type AccountId = u64;
pub type BlockNumber = u64;
pub type Balance = u64;

impl_outer_origin!{
	pub enum Origin for Test {}
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Test;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl system::Trait for Test {
    type Origin = Origin;
    type Index = u64;
    type BlockNumber = BlockNumber;
    type Call = ();
    type Hash = H256;
    type Hashing = ::sr_primitives::traits::BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type WeightMultiplierUpdate = ();
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type AvailableBlockRatio = AvailableBlockRatio;
    type MaximumBlockLength = MaximumBlockLength;
    type Version = ();
}


parameter_types! {
	pub const TransferFee: Balance = 0;
	pub const CreationFee: Balance = 0;
	pub const TransactionBaseFee: u64 = 0;
	pub const TransactionByteFee: u64 = 0;
}
impl balances::Trait for Test {
    type Balance = Balance;
    type OnFreeBalanceZero = ();
    type OnNewAccount = ();
    type Event = ();
    type TransactionPayment = ();
    type TransferPayment = ();
    type DustRemoval = ();
    type ExistentialDeposit = ();
    type TransferFee = TransferFee;
    type CreationFee = CreationFee;
    type TransactionBaseFee = TransactionBaseFee;
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = ();
}

impl token::Trait for Test {
    type Token = token::Module<Self>;
    type Event = ();
}

impl Trait for Test {
    type Event = ();
}

pub type System = system::Module<Test>;
pub type Balances = balances::Module<Test>;

use crate::Module;
pub type Dex = Module<Test>;
pub type TokenT = token::Module<Test>;


#[derive(Decode, Encode, Clone, Default)]
struct TestOrder{
    nodeid:u64,
    data1:u64,
    data2:u64,
    data3:u64,
}

impl NodeT for TestOrder
{
    type Index = u128;

    fn index(&self) -> Self::Index {
        self.nodeid as u128
    }
}

type Asd = Node<TestOrder>;

pub fn node_test(){
    let dlsfj = Node::<TestOrder>::default();
    let tesfd = Asd::default();
    let testorder = TestOrder::default();
    let node = Node::new(testorder);


}

pub fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
    system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}

pub fn printorder(index:u128){
    let order = Dex::order_info(index).unwrap();
    println!("====order index: {:?}===== ",order.index);
    print!("who {:?}  ",order.who);
    print!("amount {:?}  ",order.amount);
    print!("price {:?}  ",order.price);
    print!("left {:?}  ",order.left);
    print!("left {:?}  ",order.status);
    print!("fill_index {:?}  \n",order.fill_index);
}