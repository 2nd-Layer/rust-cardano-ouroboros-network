use std::env;
use std::path::PathBuf;
use cardano_ouroboros_network::protocols;
use protocols::mux_protocol::Cmd;

fn main() {
    let args: Vec<String> = env::args().collect();
    let host = &args[1];
    let port = 3001;
    let network_magic = 764824073;

    protocols::mux_protocol::start(Cmd::Ping, &PathBuf::new(), host, port, network_magic, &String::new(), &PathBuf::new(), &String::new(), &String::new());
}
