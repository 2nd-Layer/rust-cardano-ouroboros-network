# Cardano Rust Ouroboros Network
This crate implements the networking layer for the Ouroboros blockchain protocol. This networking crate is undergoing active refactoring to use [Tokio](https://tokio.rs/) ([see Project](https://github.com/2nd-Layer/rust-cardano-ouroboros-network/projects/6)); meanwhile it may be reasonable to use [pallas](https://github.com/txpipe/pallas), specifically  [pallas-machines](https://github.com/txpipe/pallas/tree/main/pallas-machines) and [pallas-multiplexer](https://github.com/txpipe/pallas/tree/main/pallas-multiplexer) which use MSPC channels which currenlty provide more stability.

## Mini-protocol Implementation Status

| **Protocol Name** | **Implementation Status** |
|-------------------|---------------------------|
| BlockFetch        | Implemented               |
| ChainSync         | Implemented               |
| Handshake         | Implemented               |
| KeepAlive         | Not Implemented           |
| LocalStateQuery   | Not Implemented           |
| LocalTxMonitor    | Not Implemented           |
| LocalTxSubmission | Not Implemented           |
| TipSample         | Not Implemented           |
| TxSubmission      | Partially Implemented     |
| TxSubmission2     | Not Implemented           |

# Contributing

## Submit Pull Requests
This repository implements a [GitHub Action](https://github.com/aslafy-z/conventional-pr-title-action) to make sure that the PR name follows [Conventional Commits specification](https://www.conventionalcommits.org/en/v1.0.0/). Make sure your PRs follow the referred specification to pass Continuous Integration testing.

## [Cardano Project Catalyst](https://cardano.ideascale.com/a/home)
We regularly apply for funding from individual funds of [Cardano Project Catalyst](https://cardano.ideascale.com/a/home), below you can see our history of applications for funding:

### Closed Funding Rounds
  - [ ] [Fund 6 - Ouroboros Rust Networking Crate](https://cardano.ideascale.com/a/dtd/Rust-Cardano-Networking-Crate/367442-48088), project not funded
    - 14 000 USD requested, but was over budget for Fund 6
  - [ ] [Fund 3 - Ouroboros Rust Networking Crate](https://cardano.ideascale.com/a/dtd/Ouroboros-Networking-Rust-Crate/333161-48088), project not funded
    - 7 000 USD requested, but was over budget for Fund 3

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

## Emurgo CDDL CodeGen
  - [Emurgo/cddl-codegen](https://github.com/Emurgo/cddl-codegen)

  Codegen serialization logic for CBOR automatically from a CDDL specification.

  Instead of hand-writing CBOR code and writing tests to make sure it matches your CDDL spec, it's much faster to just generate the code from the spec! It will save time and make it easier to keep all your code in sync with any changes to your specification.

  You can learn more about [CDDL here](https://github.com/cbor-wg/cddl).

## Reference implementations
  - [Haskell Cardano Node Ouroboros Network Framework](https://github.com/input-output-hk/ouroboros-network/tree/master/ouroboros-network-framework)
  - [Haskell Cardano Node Ouroboros Network](https://github.com/input-output-hk/ouroboros-network/tree/master/ouroboros-network)
  - [~~Rust Sub-set of Cardano Node Ouroboros Network Protocols~~](https://github.com/AndrewWestberg/cncli/tree/develop/src/nodeclient/protocols)
    - [CNCLI](https://github.com/AndrewWestberg/cncli) now uses this Rust crate for networking layer
  - [Pallas - Rust-native building blocks for the Cardano blockchain ecosystem](https://github.com/txpipe/pallas)

# License

This project is licensed under:
  - Mozilla Public License 2.0 (LICENSE or https://spdx.org/licenses/MPL-2.0.html)

  If for some reason you need different license, please [open an issue](https://github.com/2nd-Layer/rust-cardano-ouroboros-network/issues), we will evaluate your request for project-specific licensing.
