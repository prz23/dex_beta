//test

use super::*;
use runtime_io::with_externalities;
use sr_primitives::traits::OnInitialize;
use support::{assert_ok, assert_noop, assert_eq_uvec, StorageLinkedMap,StorageMap};
use mock::*;
use support::traits::{Currency, ReservableCurrency};



#[test]
fn basic_setup_works() {
    // Verifies initial conditions of mock
    with_externalities(&mut new_test_ext(), || {


                       });
}

