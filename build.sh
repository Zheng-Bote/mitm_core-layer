#!/usr/bin/sh

echo "Building core-layer components (static musl)..."

# Ensure the musl target is installed
rustup target add x86_64-unknown-linux-musl

# Build the workspace
cargo build --release --target x86_64-unknown-linux-musl

# Create bin directory if it doesn't exist
mkdir -p ../bin

# Copy the binaries
echo "Copying binaries to ../app/bin/"
cp target/x86_64-unknown-linux-musl/release/mitm-http-server ../app/bin/mitm-core-http
cp target/x86_64-unknown-linux-musl/release/mitm-iam-server ../app/bin/mitm-core-iam
cp target/x86_64-unknown-linux-musl/release/mitm-scheduler-server ../app/bin/mitm-core-scheduler

echo "Build complete! Binaries are located in ../app/bin/"
