/**
Forked-off from https://github.com/AndrewWestberg/cncli/ on 2020-11-30
© 2020 Andrew Westberg licensed under Apache-2.0

Re-licensed under GPLv3 or LGPLv3
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use std::{
    io,
    path::PathBuf,
};
use log::debug;
use blake2b_simd::Params;
use rusqlite::{Connection, Error, named_params, NO_PARAMS};
use cardano_ouroboros_network::{
    BlockStore,
    storage::msg_roll_forward::MsgRollForward,
};

pub struct SQLiteBlockStore {
    pub db: Connection,
}

impl SQLiteBlockStore {
    const DB_VERSION: i64 = 2;

    pub fn new(db_path: &PathBuf) -> Result<SQLiteBlockStore, Error> {
        debug!("Opening database");
        let db = Connection::open(db_path)?;
        {
            debug!("Intialize database.");
            db.execute_batch("PRAGMA journal_mode=WAL")?;
            db.execute("CREATE TABLE IF NOT EXISTS db_version (version INTEGER PRIMARY KEY)", NO_PARAMS)?;
            let mut stmt = db.prepare("SELECT version FROM db_version")?;
            let mut rows = stmt.query(NO_PARAMS)?;
            let version: i64 = match rows.next()? {
                None => { -1 }
                Some(row) => {
                    row.get(0)?
                }
            };

            // Upgrade their database to version 1
            if version < 1 {
                debug!("Upgrade database to version 1...");
                db.execute("CREATE TABLE IF NOT EXISTS chain (\
                    id INTEGER PRIMARY KEY AUTOINCREMENT, \
                    block_number INTEGER NOT NULL, \
                    slot_number INTEGER NOT NULL, \
                    hash TEXT NOT NULL, \
                    prev_hash TEXT NOT NULL, \
                    eta_v TEXT NOT NULL, \
                    node_vkey TEXT NOT NULL, \
                    node_vrf_vkey TEXT NOT NULL, \
                    eta_vrf_0 TEXT NOT NULL, \
                    eta_vrf_1 TEXT NOT NULL, \
                    leader_vrf_0 TEXT NOT NULL, \
                    leader_vrf_1 TEXT NOT NULL, \
                    block_size INTEGER NOT NULL, \
                    block_body_hash TEXT NOT NULL, \
                    pool_opcert TEXT NOT NULL, \
                    unknown_0 INTEGER NOT NULL, \
                    unknown_1 INTEGER NOT NULL, \
                    unknown_2 TEXT NOT NULL, \
                    protocol_major_version INTEGER NOT NULL, \
                    protocol_minor_version INTEGER NOT NULL, \
                    orphaned INTEGER NOT NULL DEFAULT 0 \
                    )", NO_PARAMS)?;
                db.execute("CREATE INDEX IF NOT EXISTS idx_chain_slot_number ON chain(slot_number)", NO_PARAMS)?;
                db.execute("CREATE INDEX IF NOT EXISTS idx_chain_orphaned ON chain(orphaned)", NO_PARAMS)?;
                db.execute("CREATE INDEX IF NOT EXISTS idx_chain_hash ON chain(hash)", NO_PARAMS)?;
                db.execute("CREATE INDEX IF NOT EXISTS idx_chain_block_number ON chain(block_number)", NO_PARAMS)?;
            }

            // Upgrade their database to version 2
            if version < 2 {
                debug!("Upgrade database to version 2...");
                db.execute("CREATE TABLE IF NOT EXISTS slots (\
                    id INTEGER PRIMARY KEY AUTOINCREMENT, \
                    epoch INTEGER NOT NULL, \
                    pool_id TEXT NOT NULL, \
                    slot_qty INTEGER NOT NULL, \
                    slots TEXT NOT NULL, \
                    hash TEXT NOT NULL,
                    UNIQUE(epoch,pool_id)
                )", NO_PARAMS)?;
            }

            // Update the db version now that we've upgraded the user's database fully
            if version < 0 {
                db.execute("INSERT INTO db_version (version) VALUES (?1)", &[&SQLiteBlockStore::DB_VERSION])?;
            } else {
                db.execute("UPDATE db_version SET version=?1", &[&SQLiteBlockStore::DB_VERSION])?;
            }
        }

        Ok(SQLiteBlockStore { db: db })
    }

    fn sql_save_block(&mut self, pending_blocks: &mut Vec<MsgRollForward>, network_magic: u32) -> Result<(), rusqlite::Error> {
        let db = &mut self.db;

        // get the last block eta_v (nonce) in the db
        let mut prev_eta_v =
            {
                hex::decode(
                    match db.query_row("SELECT eta_v, max(slot_number) FROM chain WHERE orphaned = 0", NO_PARAMS, |row| row.get(0)) {
                        Ok(eta_v) => { eta_v }
                        Err(_) => {
                            if network_magic == 764824073 {
                                // mainnet genesis hash
                                String::from("1a3be38bcbb7911969283716ad7aa550250226b76a61fc51cc9a9a35d9276d81")
                            } else {
                                // assume testnet genesis hash
                                String::from("849a1764f152e1b09c89c0dfdbcbdd38d711d1fec2db5dfa0f87cf2737a0eaf4")
                            }
                        }
                    }
                ).unwrap()
            };

        let tx = db.transaction()?;
        { // scope for db transaction
            let mut orphan_stmt = tx.prepare("UPDATE chain SET orphaned = 1 WHERE block_number >= ?1")?;
            let mut insert_stmt = tx.prepare("INSERT INTO chain (\
            block_number, \
            slot_number, \
            hash, \
            prev_hash, \
            eta_v, \
            node_vkey, \
            node_vrf_vkey, \
            eta_vrf_0, \
            eta_vrf_1, \
            leader_vrf_0, \
            leader_vrf_1, \
            block_size, \
            block_body_hash, \
            pool_opcert, \
            unknown_0, \
            unknown_1, \
            unknown_2, \
            protocol_major_version, \
            protocol_minor_version) \
            VALUES (\
            :block_number, \
            :slot_number, \
            :hash, \
            :prev_hash, \
            :eta_v, \
            :node_vkey, \
            :node_vrf_vkey, \
            :eta_vrf_0, \
            :eta_vrf_1, \
            :leader_vrf_0, \
            :leader_vrf_1, \
            :block_size, \
            :block_body_hash, \
            :pool_opcert, \
            :unknown_0, \
            :unknown_1, \
            :unknown_2, \
            :protocol_major_version, \
            :protocol_minor_version)")?;

            for block in pending_blocks.drain(..) {
                // Set any necessary blocks as orphans
                let orphan_num = orphan_stmt.execute(&[&block.block_number])?;

                if orphan_num > 0 {
                    // get the last block eta_v (nonce) in the db
                    prev_eta_v = {
                        hex::decode(
                            match tx.query_row("SELECT eta_v, max(slot_number) FROM chain WHERE orphaned = 0", NO_PARAMS, |row| row.get(0)) {
                                Ok(eta_v) => { eta_v }
                                Err(_) => {
                                    if network_magic == 764824073 {
                                        // mainnet genesis hash
                                        String::from("1a3be38bcbb7911969283716ad7aa550250226b76a61fc51cc9a9a35d9276d81")
                                    } else {
                                        // assume testnet genesis hash
                                        String::from("849a1764f152e1b09c89c0dfdbcbdd38d711d1fec2db5dfa0f87cf2737a0eaf4")
                                    }
                                }
                            }
                        ).unwrap()
                    };
                }
                // blake2b hash of eta_vrf_0
                let mut block_eta_v = Params::new().hash_length(32).to_state().update(&*block.eta_vrf_0).finalize().as_bytes().to_vec();
                prev_eta_v.append(&mut block_eta_v);
                // blake2b hash of prev_eta_v + block_eta_v
                prev_eta_v = Params::new().hash_length(32).to_state().update(&*prev_eta_v).finalize().as_bytes().to_vec();

                insert_stmt.execute_named(
                    named_params! {
                    ":block_number" : block.block_number,
                    ":slot_number": block.slot_number,
                    ":hash" : hex::encode(block.hash),
                    ":prev_hash" : hex::encode(block.prev_hash),
                    ":eta_v" : hex::encode(&prev_eta_v),
                    ":node_vkey" : hex::encode(block.node_vkey),
                    ":node_vrf_vkey" : hex::encode(block.node_vrf_vkey),
                    ":eta_vrf_0" : hex::encode(block.eta_vrf_0),
                    ":eta_vrf_1" : hex::encode(block.eta_vrf_1),
                    ":leader_vrf_0" : hex::encode(block.leader_vrf_0),
                    ":leader_vrf_1" : hex::encode(block.leader_vrf_1),
                    ":block_size" : block.block_size,
                    ":block_body_hash" : hex::encode(block.block_body_hash),
                    ":pool_opcert" : hex::encode(block.pool_opcert),
                    ":unknown_0" : block.unknown_0,
                    ":unknown_1" : block.unknown_1,
                    ":unknown_2" : hex::encode(block.unknown_2),
                    ":protocol_major_version" : block.protocol_major_version,
                    ":protocol_minor_version" : block.protocol_minor_version,
                }
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

}

impl BlockStore for SQLiteBlockStore {
    fn save_block(&mut self, mut pending_blocks: &mut Vec<MsgRollForward>, network_magic: u32) -> io::Result<()> {
        match self.sql_save_block(&mut pending_blocks, network_magic) {
            Ok(_) => Ok(()),
            Err(_) => Err(io::Error::new(io::ErrorKind::Other, "Database error!")),
        }
    }

    fn load_blocks(&mut self) -> Option<Vec<(i64, Vec<u8>)>> {
        let db = &self.db;
        let mut stmt = db.prepare("SELECT slot_number, hash FROM chain where orphaned = 0 ORDER BY slot_number DESC LIMIT 33").unwrap();
        let blocks = stmt.query_map(NO_PARAMS, |row| {
            let slot_result: Result<i64, Error> = row.get(0);
            let hash_result: Result<String, Error> = row.get(1);
            let slot = slot_result?;
            let hash = hash_result?;
            Ok((slot, hex::decode(hash).unwrap()))
        }).ok()?;
        Some(blocks.map(|item| item.unwrap()).collect())
    }
}
