#!/bin/bash
set -e

#  © 2020 PERLUR Group
#
# SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

if ! which jq >> /dev/null 2>&1; then
  echo "ERROR: jq binary is missing!"
  exit 0
fi
if ! which docker >> /dev/null 2>&1; then
  echo "ERROR: docker binary is missing!"
  exit 0
fi
if ! cd ${PWD}/.github/tests/00-cardano-node-integration-tests/; then
  echo "ERROR: Failed to switch to cardano-node-integration-tests directory!"
fi

GH_JSON=$(curl --proto '=https' --tlsv1.2 -sSf "https://api.github.com/repos/input-output-hk/cardano-node/releases/latest")
if [ $(jq -r .prerelease <<< ${GH_JSON}) == false ]; then
  cardanoNodeVersionTag=$(jq -r .tag_name <<< ${GH_JSON})
  echo "Discovered Cardano Node ${cardanoNodeVersionTag}"
  cardanoNodeVersion=${cardanoNodeVersionTag}
fi

sed -i "s/<cardanoNodeVersionTag>/${cardanoNodeVersionTag}/" \
  ${PWD}/Dockerfile

echo "Pull Docker image from Docker Hub"
if ! docker pull 2ndlayer/centos-cardano-node:${cardanoNodeVersion} >> /dev/null 2>&1; then
  echo "ERROR: Docker image pull failed!"
  exit 0
else TESTNET_MAGIC=${RANDOM}
  docker build -t local/cardano-node-shelley-testnet:${cardanoNodeVersion} ./

  if docker run local/cardano-node-shelley-testnet:${cardanoNodeVersion} \
    bash /usr/local/lib/cardano-node-integration-testing-create-testnet.sh;
  then
    CONTAINER_ID=$(docker ps -aq | head -n 1)
    if docker commit ${CONTAINER_ID} local/cardano-node-shelley-testnet:${cardanoNodeVersion}; then
      echo "INFO: Initial testing Shelley environment commited!"
    fi
    if docker run \
      local/cardano-node-shelley-testnet:${cardanoNodeVersion} \
      bash /usr/local/lib/cardano-node-integration-testing-update-genesis.sh;
    then
      echo "INFO: Testnet INIT scripts executed!"
      if docker commit ${CONTAINER_ID} local/cardano-node-shelley-testnet:${cardanoNodeVersion}; then
        echo "DEBUG: Image name: local/cardano-node-shelley-testnet:${cardanoNodeVersion}"
      fi
    else
      echo "ERROR: Failed to execute testnet INIT scripts!"
      exit 0
    fi
  else
    echo "ERROR: Failed to create Shelley testing environment!"
    exit 0
  fi
fi