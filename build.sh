#!/bin/bash

# Build script for seedbrr that automatically copies the binary 2 directories up

echo "Building seedbrr..."

# Build in release mode
cargo build --release

# Check if build was successful
if [ $? -eq 0 ]; then
    echo "Build successful!"
    
    # Copy the binary 
    cp -f target/release/seedbrr seedbrr
    echo "Copied binary to: seedbrr"
    
    # Make sure it's executable
    chmod +x seedbrr
    
    echo "Build complete! You can now run seedbrr"
else
    echo "Build failed!"
    exit 1
fi