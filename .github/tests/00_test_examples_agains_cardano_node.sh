#!/bin/bash
set -e

if ! which jq >> /dev/null 2>&1; then
  echo "ERROR: jq binary is missing!"
  exit 0
elif ! which docker >> /dev/null 2>&1; then
  echo "ERROR: docker binary is missing!"
  exit 0
fi

GH_JSON=$(curl --proto '=https' --tlsv1.2 -sSf "https://api.github.com/repos/input-output-hk/cardano-node/releases/latest")
if [ $(jq -r .prerelease <<< ${GH_JSON} ) == false ]; then
  cardanoNodeVersionTag=$(jq -r .tag_name <<< ${GH_JSON})
  echo "Discovered Cardano Node ${cardanoNodeVersionTag}"
  cardanoNodeVersion=${cardanoNodeVersionTag}
fi

echo "Pull Docker image from Docker Hub"
if ! docker pull 2ndlayer/centos-cardano-node:${cardanoNodeVersion} >> /dev/null 2>&1; then
  echo "ERROR: Docker image pull failed!"
  exit 0
else
  TESTNET_MAGIC=${RANDOM}
  if docker run 2ndlayer/centos-cardano-node:${cardanoNodeVersion} \
    cardano-cli genesis create-staked \
      --genesis-dir testnet \
      --gen-genesis-keys 2 \
      --gen-utxo-keys 1 \
      --gen-stake-delegs 2 \
      --supply 100000 \
      --supply-delegated 70000 \
      --gen-pools 2 \
      --testnet-magic ${TESTNET_MAGIC}; 
    then
      echo "INFO: New testing Shelley environment created!"
    else
      echo "ERROR: Failed to create testing Shelley environment!"
  fi
fi