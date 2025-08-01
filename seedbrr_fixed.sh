#!/bin/bash

# Define lockfile path
LOCKFILE="/tmp/seedbrr_$$.lock"

# Create lock file
echo $$ > "$LOCKFILE"

# Clean up lock file on exit
trap 'rm -f "$LOCKFILE"' EXIT

# Log environment variables and working directory for debugging
sleep 3
export PATH="$HOME/.local/bin:$PATH"
echo "Environment variables:" >> /home/beholder/seedbrr/debug.log
env >> /home/beholder/seedbrr/debug.log
echo "Current working directory: $(pwd)" >> /home/beholder/seedbrr/debug.log
echo "Arguments: $@" >> /home/beholder/seedbrr/debug.log

# Remove empty arguments
ARGS=("$@")
CLEAN_ARGS=()
for ARG in "${ARGS[@]}"; do
    if [ -n "$ARG" ]; then
        CLEAN_ARGS+=("$ARG")
    fi
done

# Check if the last argument is a 4-digit custom category/type (only if we have arguments)
if [ ${#CLEAN_ARGS[@]} -gt 0 ] && [[ "${CLEAN_ARGS[-1]}" =~ ^[0-9]{4}$ ]]; then
    CUSTOM_CAT_TYPE="${CLEAN_ARGS[-1]}" # Extract the 4-digit value
    unset CLEAN_ARGS[-1]               # Remove it from the arguments
    CLEAN_ARGS+=("-c" "$CUSTOM_CAT_TYPE") # Add it as the `-c` argument
    echo "Detected custom category/type: $CUSTOM_CAT_TYPE" >> /home/beholder/seedbrr/debug.log
fi

# Set the working directory to the script's location
cd /home/beholder/seedbrr || exit 1
echo "Changed directory to: $(pwd)" >> /home/beholder/seedbrr/debug.log

# Launch the Rust binary with cleaned arguments and timeout protection
echo "Launching Rust binary with arguments: ${CLEAN_ARGS[@]}" >> /home/beholder/seedbrr/debug.log
timeout 1800 ./seedbrr "${CLEAN_ARGS[@]}" >> /home/beholder/seedbrr/debug.log 2>&1
EXIT_CODE=$?

if [ $EXIT_CODE -eq 124 ]; then
    echo "Rust binary timed out after 30 minutes" >> /home/beholder/seedbrr/debug.log
elif [ $EXIT_CODE -eq 0 ]; then
    echo "Rust binary finished successfully" >> /home/beholder/seedbrr/debug.log
else
    echo "Rust binary exited with code: $EXIT_CODE" >> /home/beholder/seedbrr/debug.log
fi