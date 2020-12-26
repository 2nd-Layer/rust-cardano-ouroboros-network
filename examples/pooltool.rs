use cardano_ouroboros_network::{
    storage::msg_roll_forward::MsgRollForward,
    protocols::chainsync::Listener,
};
use std::path::PathBuf;
use log::{info, error};
use serde::Serialize;
use regex::Regex;
use chrono::{SecondsFormat, Utc};
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
    ops::Sub,
};

struct PoolTool {
    pub pooltool_api_key: String,
    pub cardano_node_path: PathBuf,
    pub node_version: String,
    pub last_node_version_time: Instant,
    pub pool_name: String,
    pub pool_id: String,
}

impl Default for PoolTool {
    fn default() -> Self {
        PoolTool {
            pooltool_api_key: String::new(),
            cardano_node_path: PathBuf::new(),
            node_version: String::new(),
            last_node_version_time: Instant::now().sub(Duration::from_secs(7200)), // 2 hours ago
            pool_name: String::new(),
            pool_id: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PooltoolStats {
    api_key: String,
    pool_id: String,
    data: PooltoolData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PooltoolData {
    node_id: String,
    version: String,
    at: String,
    block_no: i64,
    slot_no: i64,
    block_hash: String,
    parent_hash: String,
    leader_vrf: String,
    platform: String,
}

impl Listener for PoolTool {
    fn handle_tip(&mut self, header: &MsgRollForward) {
        if self.last_node_version_time.elapsed() > Duration::from_secs(3600) {
            // Our node version is outdated. Make a call to update it.
            let output = Command::new(&self.cardano_node_path)
                .arg("--version")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .output()
                .expect(&*format!("Failed to execute {:?}", &self.cardano_node_path));
            let version_string = String::from_utf8_lossy(&output.stdout);
            let cap = Regex::new("cardano-node (\\d+\\.\\d+\\.\\d+) .*\ngit rev ([a-f0-9]{5}).*").unwrap().captures(&*version_string).unwrap();
            self.node_version = format!("{}:{}", cap.get(1).map_or("", |m| m.as_str()), cap.get(2).map_or("", |m| m.as_str()));
            info!("Checking cardano-node version: {}", &self.node_version);
            self.last_node_version_time = Instant::now();
        }
        let client = reqwest::blocking::Client::new();
        let pooltool_result = client.post("https://api.pooltool.io/v0/sendstats").body(
            serde_json::ser::to_string(
                &PooltoolStats {
                    api_key: self.pooltool_api_key.clone(),
                    pool_id: self.pool_id.clone(),
                    data: PooltoolData {
                        node_id: "".to_string(),
                        version: self.node_version.clone(),
                        at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                        block_no: header.block_number,
                        slot_no: header.slot_number,
                        block_hash: hex::encode(&header.hash),
                        parent_hash: hex::encode(&header.prev_hash),
                        leader_vrf: hex::encode(&header.leader_vrf_0),
                        platform: "cncli".to_string(),
                    },
                }
            ).unwrap()
        ).send();

        match pooltool_result {
            Ok(response) => {
                match response.text() {
                    Ok(text) => {
                        info!("Pooltool ({}, {}): ({}, {}), json: {}", &self.pool_name, &self.pool_id[..8], &header.block_number, hex::encode(&header.hash[..8]), text);
                    }
                    Err(error) => { error!("PoolTool error: {}", error); }
                }
            }
            Err(error) => { error!("PoolTool error: {}", error); }
        }
    }
}
