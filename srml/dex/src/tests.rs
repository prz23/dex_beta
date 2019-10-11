//test

use super::*;
use runtime_io::with_externalities;
use sr_primitives::traits::OnInitialize;
use support::{assert_ok, assert_noop, assert_eq_uvec, StorageLinkedMap,StorageMap};
use mock::*;
use support::traits::{Currency, ReservableCurrency};



#[test]
fn basic_match_works() {
    // Verifies initial conditions of mock
    with_externalities(&mut new_test_ext(), || {
        // add a tokentype
        let tokentype = vec![1u8,2u8];
        let tokentype2 = vec![3u8,4u8];
        TokenT::add_new_tokentype(Origin::signed(1),tokentype.clone(),1000);
        TokenT::add_new_tokentype(Origin::signed(1),tokentype2.clone(),1000);
        assert_eq!(TokenT::token_type_and_precision(tokentype.clone()),Some(1000));
        assert_eq!(TokenT::token_type_and_precision(tokentype2.clone()),Some(1000));

        // mint 10000 token of tokentype to AccountId -> 10
        TokenT::depositing_token(&10,tokentype.clone(),10000);
        assert_eq!(TokenT::free_token((tokentype.clone(),10)),10000);
        // mint 10000 token of tokentype to AccountId -> 10
        TokenT::depositing_token(&11,tokentype2.clone(),10000);
        assert_eq!(TokenT::free_token((tokentype2.clone(),11)),10000);

        let order_pair:OrderPair = OrderPair{
            first:tokentype.clone(),
            second:tokentype2.clone(),
        };

        //add_new_order_pair
        assert_ok!(Dex::add_new_order_pair(order_pair.clone()));

        assert_eq!(Dex::put_order_and_match(Origin::signed(10),order_pair.clone(),OrderType::Buy,100,100),Err("Insufficient available balance"));

        assert_ok!(Dex::put_order_and_match(Origin::signed(10),order_pair.clone(),OrderType::Sell,100,100));
        assert_eq!(TokenT::free_token((tokentype.clone(),10)),9900);
        assert_eq!(TokenT::locked_token((tokentype.clone(),10)),100);

        assert_ok!(Dex::put_order_and_match(Origin::signed(11),order_pair.clone(),OrderType::Buy,50,105));
        assert_eq!(TokenT::free_token((tokentype2.clone(),11)),5000);
        assert_eq!(TokenT::locked_token((tokentype2.clone(),11)),0);  // 5250 - 5000 = 250 多出250怎么办
        // 需要增加算法 当前交易 额外解锁 =》 交易数量amount*（挂单价格 - 实际交易价格）

        assert_eq!(TokenT::free_token((tokentype.clone(),11)),50); // buyer get 50
        assert_eq!(TokenT::locked_token((tokentype.clone(),10)),50); //left seller
        assert_eq!(TokenT::free_token((tokentype2.clone(),10)),5000); // get money

    });
}


#[test]
fn muilt_match_test() {
    // Verifies initial conditions of mock
    with_externalities(&mut new_test_ext(), || {
        // add a tokentype
        let tokentype = vec![1u8,2u8];
        let tokentype2 = vec![3u8,4u8];
        TokenT::add_new_tokentype(Origin::signed(1),tokentype.clone(),1000);
        TokenT::add_new_tokentype(Origin::signed(1),tokentype2.clone(),1000);
        assert_eq!(TokenT::token_type_and_precision(tokentype.clone()),Some(1000));
        assert_eq!(TokenT::token_type_and_precision(tokentype2.clone()),Some(1000));

        // mint 10000 token of tokentype to AccountId -> 10
        TokenT::depositing_token(&10,tokentype.clone(),10000);
        assert_eq!(TokenT::free_token((tokentype.clone(),10)),10000);
        // mint 10000 token of tokentype2 to AccountId -> 11
        TokenT::depositing_token(&11,tokentype2.clone(),10000);
        assert_eq!(TokenT::free_token((tokentype2.clone(),11)),10000);

        let order_pair:OrderPair = OrderPair{
            first:tokentype.clone(),
            second:tokentype2.clone(),
        };

        //add_new_order_pair
        assert_ok!(Dex::add_new_order_pair(order_pair.clone()));

        assert_eq!(Dex::put_order_and_match(Origin::signed(10),order_pair.clone(),OrderType::Buy,100,100),Err("Insufficient available balance"));

        assert_ok!(Dex::put_order_and_match(Origin::signed(10),order_pair.clone(),OrderType::Sell,100,100));
        assert_eq!(TokenT::free_token((tokentype.clone(),10)),9900);
        assert_eq!(TokenT::locked_token((tokentype.clone(),10)),100);

        assert_ok!(Dex::put_order_and_match(Origin::signed(11),order_pair.clone(),OrderType::Buy,50,95));
        assert_ok!(Dex::put_order_and_match(Origin::signed(11),order_pair.clone(),OrderType::Buy,50,105));
        assert_eq!(TokenT::free_token((tokentype2.clone(),11)),250);
        assert_eq!(TokenT::locked_token((tokentype2.clone(),11)),4750);  // 50x95=4750
        // 需要增加算法 当前交易 额外解锁 =》 交易数量amount*（挂单价格 - 实际交易价格）

        assert_eq!(TokenT::free_token((tokentype.clone(),11)),50); // buyer get 50
        assert_eq!(TokenT::locked_token((tokentype.clone(),10)),50); //left seller
        assert_eq!(TokenT::free_token((tokentype2.clone(),10)),5000); // get money

        assert_ok!(Dex::put_order_and_match(Origin::signed(10),order_pair.clone(),OrderType::Sell,100,105));
        assert_eq!(TokenT::free_token((tokentype.clone(),10)),9800);
        assert_eq!(TokenT::locked_token((tokentype.clone(),10)),150);
        //now 2 sell order  amount/price 100/105 and 50/100

        // mint 30000 token of tokentype2 to AccountId -> 12
        TokenT::depositing_token(&12,tokentype2.clone(),30000);
        assert_eq!(TokenT::free_token((tokentype2.clone(),12)),30000);

        // id 12 buy 105 with price 120
        assert_ok!(Dex::put_order_and_match(Origin::signed(12),order_pair.clone(),OrderType::Buy,105,120));
        assert_eq!(TokenT::free_token((tokentype2.clone(),12)),19225);//30000 - 5775 - 5000 = 19225
        assert_eq!(TokenT::free_token((tokentype.clone(),12)),105);  // get share 105
        assert_eq!(TokenT::locked_token((tokentype2.clone(),12)),0); // overlocked tokens are returned

        printorder(1);
        printorder(2);
        printorder(3);
        printorder(4);
        printorder(5);
    });
}

#[test]
fn cancel_test() {
    // Verifies initial conditions of mock
    with_externalities(&mut new_test_ext(), || {
        // add a tokentype
        let tokentype = vec![1u8,2u8];
        let tokentype2 = vec![3u8,4u8];
        TokenT::add_new_tokentype(Origin::signed(1),tokentype.clone(),1000);
        TokenT::add_new_tokentype(Origin::signed(1),tokentype2.clone(),1000);
        assert_eq!(TokenT::token_type_and_precision(tokentype.clone()),Some(1000));
        assert_eq!(TokenT::token_type_and_precision(tokentype2.clone()),Some(1000));

        // mint 10000 token of tokentype to AccountId -> 10
        TokenT::depositing_token(&10,tokentype.clone(),10000);
        assert_eq!(TokenT::free_token((tokentype.clone(),10)),10000);
        // mint 10000 token of tokentype2 to AccountId -> 11
        TokenT::depositing_token(&11,tokentype2.clone(),10000);
        assert_eq!(TokenT::free_token((tokentype2.clone(),11)),10000);

        let order_pair:OrderPair = OrderPair{
            first:tokentype.clone(),
            second:tokentype2.clone(),
        };

        //add_new_order_pair
        assert_ok!(Dex::add_new_order_pair(order_pair.clone()));

        assert_eq!(Dex::put_order_and_match(Origin::signed(10),order_pair.clone(),OrderType::Buy,100,100),Err("Insufficient available balance"));

        assert_ok!(Dex::put_order_and_match(Origin::signed(10),order_pair.clone(),OrderType::Sell,100,100));
        assert_eq!(TokenT::free_token((tokentype.clone(),10)),9900);
        assert_eq!(TokenT::locked_token((tokentype.clone(),10)),100);

        assert_ok!(Dex::put_order_and_match(Origin::signed(11),order_pair.clone(),OrderType::Buy,50,95));
        assert_ok!(Dex::put_order_and_match(Origin::signed(11),order_pair.clone(),OrderType::Buy,50,105));
        assert_eq!(TokenT::free_token((tokentype2.clone(),11)),250);
        assert_eq!(TokenT::locked_token((tokentype2.clone(),11)),4750);  // 50x95=4750
        // 需要增加算法 当前交易 额外解锁 =》 交易数量amount*（挂单价格 - 实际交易价格）

        assert_eq!(TokenT::free_token((tokentype.clone(),11)),50); // buyer get 50
        assert_eq!(TokenT::locked_token((tokentype.clone(),10)),50); //left seller
        assert_eq!(TokenT::free_token((tokentype2.clone(),10)),5000); // get money

        assert_ok!(Dex::put_order_and_match(Origin::signed(10),order_pair.clone(),OrderType::Sell,100,105));
        assert_eq!(TokenT::free_token((tokentype.clone(),10)),9800);
        assert_eq!(TokenT::locked_token((tokentype.clone(),10)),150);
        //now 2 sell order  amount/price 100/105 and 50/100

        // mint 30000 token of tokentype2 to AccountId -> 12
        TokenT::depositing_token(&12,tokentype2.clone(),30000);
        assert_eq!(TokenT::free_token((tokentype2.clone(),12)),30000);

        // id 12 buy 105 with price 120
        assert_ok!(Dex::put_order_and_match(Origin::signed(12),order_pair.clone(),OrderType::Buy,105,120));
        assert_eq!(TokenT::free_token((tokentype2.clone(),12)),19225);//30000 - 5775 - 5000 = 19225
        assert_eq!(TokenT::free_token((tokentype.clone(),12)),105);  // get share 105
        assert_eq!(TokenT::locked_token((tokentype2.clone(),12)),0); // overlocked tokens are returned

        printorder(1);
        printorder(2);
        printorder(3);
        printorder(4);
        printorder(5);

        println!("~~~~~~NOW cancel order index 2~~~~~~");
        assert_eq!(Dex::cancel_order(Origin::signed(12),order_pair.clone(),2),Err("not permitted"));
        assert_ok!(Dex::cancel_order(Origin::signed(11),order_pair.clone(),2));
        printorder(2);
    });
}

#[test]
fn full_match_test() {
    // Verifies initial conditions of mock
    with_externalities(&mut new_test_ext(), || {
        // add a tokentype
        let tokentype = vec![1u8,2u8];
        let tokentype2 = vec![3u8,4u8];
        TokenT::add_new_tokentype(Origin::signed(1),tokentype.clone(),1000);
        TokenT::add_new_tokentype(Origin::signed(1),tokentype2.clone(),1000);
        let order_pair:OrderPair = OrderPair{
            first:tokentype.clone(),
            second:tokentype2.clone(),
        };
        assert_ok!(Dex::add_new_order_pair(order_pair.clone()));

        TokenT::depositing_token(&10,tokentype.clone(),20000);
        assert_eq!(TokenT::free_token((tokentype.clone(),10)),20000);
        TokenT::depositing_token(&11,tokentype2.clone(),20000);
        assert_eq!(TokenT::free_token((tokentype2.clone(),11)),20000);
        TokenT::depositing_token(&12,tokentype.clone(),20000);
        assert_eq!(TokenT::free_token((tokentype.clone(),12)),20000);
        TokenT::depositing_token(&13,tokentype2.clone(),20000);
        assert_eq!(TokenT::free_token((tokentype2.clone(),13)),20000);
        TokenT::depositing_token(&14,tokentype.clone(),20000);
        assert_eq!(TokenT::free_token((tokentype.clone(),14)),20000);
        TokenT::depositing_token(&15,tokentype2.clone(),20000);
        assert_eq!(TokenT::free_token((tokentype2.clone(),15)),20000);
        TokenT::depositing_token(&16,tokentype2.clone(),20000);
        assert_eq!(TokenT::free_token((tokentype2.clone(),16)),20000);

        println!("1");
        assert_ok!(Dex::put_order_and_match(Origin::signed(11),order_pair.clone(),OrderType::Buy,105,120));
        printorder(1);
        println!("2");
        assert_ok!(Dex::put_order_and_match(Origin::signed(10),order_pair.clone(),OrderType::Sell,100,120));
        printorder(1);
        printorder(2);
        println!("3");
        assert_ok!(Dex::put_order_and_match(Origin::signed(12),order_pair.clone(),OrderType::Sell,100,120));
        printorder(1);
        printorder(2);
        printorder(3);
        println!("4");
        assert_ok!(Dex::put_order_and_match(Origin::signed(13),order_pair.clone(),OrderType::Buy,105,120));
        printorder(1);
        printorder(2);
        printorder(3);
        printorder(4);
        TokenT::depositing_token(&13,tokentype2.clone(),20000);
        assert_ok!(Dex::put_order_and_match(Origin::signed(13),order_pair.clone(),OrderType::Buy,105,120));
        printorder(5);
        println!("xx");
        assert_ok!(Dex::put_order_and_match(Origin::signed(14),order_pair.clone(),OrderType::Sell,100,110));
        printorder(4);
        printorder(5);
        printorder(6);
    });
}
