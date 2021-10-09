/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::mux;
use futures::executor::block_on;
use log::info;

mod common;

/**
 * Test a handshake with the local node's unix socket
 */
fn main() {
    let _cfg = common::init();

    block_on(async {
        let channel = mux::connection::connect_unix("/home/westbam/haskell/local/db/socket")
            .await
            .unwrap();
        channel.handshake(764824073).await.unwrap();
        info!("Ping unix socket success");
    });
}
