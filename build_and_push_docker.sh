#!/bin/bash
set -e

# Extract version from Cargo.toml
VERSION=$(grep '^version' stark-backend/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Detected starkbot version: $VERSION"

echo "Building Docker image..."
docker build \
  --build-arg STARKBOT_VERSION="$VERSION" \
  -t ghcr.io/starkbotai/starkbot:flash \
  -t ghcr.io/starkbotai/starkbot:latest \
  -t "ghcr.io/starkbotai/starkbot:$VERSION" \
  .

echo "Pushing to registry..."
docker push ghcr.io/starkbotai/starkbot:flash
docker push ghcr.io/starkbotai/starkbot:latest
docker push "ghcr.io/starkbotai/starkbot:$VERSION"

echo "Done! Pushed version $VERSION"
