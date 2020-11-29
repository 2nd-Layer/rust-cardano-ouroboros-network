# Cardano Rust Ouroboros Network
This crate implements the networking layer for the Ouroboros blockchain protocol.

# Ouroboros Network Protocol Documenation
There are two documents which describe various levels of the networking layer of the Cardano Node Shelley implementation:

  - [Introduction to the design of Data Diffusion and Networking of Cardano Shelley](https://hydra.iohk.io/job/Cardano/ouroboros-network/native.docs.x86_64-linux/latest/download/1)

  This document explains the technical requirements and key constraints for the networking
  layer of the _Cardano Shelley_ implementation of _Ouroboros Praos_.  This is
  a design document.

  - [The Shelley Networking Protocol](https://hydra.iohk.io/job/Cardano/ouroboros-network/native.docs.x86_64-linux/latest/download/2)
  
  This document is a technical specification of the networking protocol.  It
  includes serialisation formats, necessary details of multiplexer and
  technical specifications of mini-protocols used by either _node-to-node_ and
  _node-to-client_ flavors of the protocol.
  
## Wireshark Plug-in
  - [Wireshark Dissector for Ouroboros-Network in Lua](https://github.com/input-output-hk/ouroboros-network/tree/master/ouroboros-network/wireshark-plugin)

## Reference implementations
  - [Haskell Cardano Node Ouroboros Network Framework](https://github.com/input-output-hk/ouroboros-network/tree/master/ouroboros-network-framework)
  - [Haskell Cardano Node Ouroboros Network](https://github.com/input-output-hk/ouroboros-network/tree/master/ouroboros-network)
  - [Rust Sub-set of Cardano Node Ouroboros Network Protocols](https://github.com/AndrewWestberg/cncli/tree/develop/src/nodeclient/protocols)

# License

This project is licensed under either of the following licenses:
  - GNU General Public License v3.0 (LICENSE or https://www.gnu.org/licenses/gpl-3.0-standalone.html)
  - GNU Lesser General Public License v3.0 (COPYING.LESSER or https://www.gnu.org/licenses/lgpl-3.0-standalone.html)
