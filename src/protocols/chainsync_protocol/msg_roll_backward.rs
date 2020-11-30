/**
Forked-off from https://github.com/AndrewWestberg/cncli/ on 2020-11-30
© 2020 Andrew Westberg licensed under Apache-2.0

Re-licensed under GPLv3 or LGPLv3
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use log::error;
use serde_cbor::Value;

pub fn parse_msg_roll_backward(cbor_array: Vec<Value>) -> i64 {
    let mut slot: i64 = 0;
    match &cbor_array[1] {
        Value::Array(block) => {
            match block[0] {
                Value::Integer(parsed_slot) => { slot = parsed_slot as i64 }
                _ => { error!("invalid cbor"); }
            }
        }
        _ => { error!("invalid cbor"); }
    }

    slot
}