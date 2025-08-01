#!/bin/bash

# Simple seedbrr wrapper for qBittorrent integration
# Handles environment setup and 4-digit category/type codes

# Set up environment for qBittorrent
export PATH="$HOME/.local/bin:$PATH"

# Change to seedbrr directory
cd /home/beholder/seedbrr || exit 1

# Process arguments
SEEDBRR_ARGS=()

# Check each argument
for arg in "$@"; do
    # If it's a 4-digit code (0741, 0316, etc.), convert to -c flag
    if [[ "$arg" =~ ^[0-9]{4}$ ]]; then
        SEEDBRR_ARGS+=("-c" "$arg")
        echo "Using category/type code: $arg"
    else
        SEEDBRR_ARGS+=("$arg")
    fi
done

# Run seedbrr with processed arguments
echo "Running: ./seedbrr ${SEEDBRR_ARGS[@]}"
exec ./seedbrr "${SEEDBRR_ARGS[@]}"