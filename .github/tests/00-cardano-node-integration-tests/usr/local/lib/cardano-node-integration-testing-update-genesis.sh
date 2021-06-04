#!/bin/bash
set -e

#  © 2020 PERLUR Group
#
# SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

# Print diagnostic information
echo "PWD:" $(pwd)

#
# Modify genesis.spec.json
#

TMP_FILE=$(mktemp)
cat testnet/genesis.spec.json | jq .epochLength=300 > ${TMP_FILE}
cat ${TMP_FILE} \
    | jq .protocolParams.decentralisationParam=0 > testnet/genesis.spec.json

rm ${TMP_FILE}

#
# Modify genesis.json
#

TMP_FILE=$(mktemp)
cat testnet/genesis.json | jq .epochLength=300 > ${TMP_FILE}
cat ${TMP_FILE} \
    | jq .protocolParams.decentralisationParam=0 > etc/genesis.json
cp ${TMP_FILE} testnet/genesis.json

rm ${TMP_FILE}