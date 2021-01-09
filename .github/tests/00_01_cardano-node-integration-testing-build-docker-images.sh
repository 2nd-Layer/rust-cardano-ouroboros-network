#!/bin/bash
set -e

#  © 2020 PERLUR Group
#
# SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

if ! which jq >> /dev/null 2>&1; then
  echo "ERROR: jq binary is missing!"
  exit 0
elif ! which docker >> /dev/null 2>&1; then
  echo "ERROR: docker binary is missing!"
  exit 0
fi

GH_JSON=$(curl --proto '=https' --tlsv1.2 -sSf "https://api.github.com/repos/input-output-hk/cardano-node/releases/latest")
if [ $(jq -r .prerelease <<< ${GH_JSON}) == false ]; then
  cardanoNodeVersionTag=$(jq -r .tag_name <<< ${GH_JSON})
  echo "Discovered Cardano Node ${cardanoNodeVersionTag}"
  cardanoNodeVersion=${cardanoNodeVersionTag}
fi

echo "Pull Docker image from Docker Hub"
if ! docker pull 2ndlayer/centos-cardano-node:${cardanoNodeVersion} >> /dev/null 2>&1; then
  echo "ERROR: Docker image pull failed!"
  exit 0
elif TESTNET_MAGIC=${RANDOM}; then
  if docker run 2ndlayer/centos-cardano-node:${cardanoNodeVersion} \
    cardano-cli genesis create \
      --genesis-dir testnet \
      --gen-utxo-keys 3 \
      --supply 100000 \
      --testnet-magic ${TESTNET_MAGIC};
    then
      if CONTAINER_ID=$(docker ps -aq | head -n 1); then
        echo "INFO: Initial testing Shelley environment created!"
        docker cp \
          ${PWD}/.github/tests/00_02_cardano-node-integration-testing-update-genesis.sh \
          ${CONTAINER_ID}:/usr/local/bin/00_02_cardano-node-integration-testing-update-genesis.sh
        echo "INFO: Testnet INIT scripts added!"
        if CONTAINER_ID=$(docker ps -aq | head -n 1); then
          if docker commit ${CONTAINER_ID} local/cardano-node-shelley-testnet:${cardanoNodeVersion}; then
            echo "DEBUG: Image name: local/cardano-node-shelley-testnet:${cardanoNodeVersion}"
          fi
        fi
        docker run \
         local/cardano-node-shelley-testnet:${cardanoNodeVersion} \
         bash /usr/local/bin/00_02_cardano-node-integration-testing-update-genesis.sh
        CONTAINER_ID=$(docker ps -aq | head -n 1)
        docker commit ${CONTAINER_ID} local/cardano-node-shelley-testnet:${cardanoNodeVersion}
      else
        echo "ERROR: Failed to create testing Shelley image!"
      fi
    else
      echo "ERROR: Failed to obtain IMAGE ID testing Shelley environment!"
      exit 0
  fi
else
  echo "ERROR: Failed to create Shellet testing environment!"
fi