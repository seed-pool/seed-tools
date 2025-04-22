#!/bin/bash
SEED_TOOLS_DIR="/home/user/seed-tools" # Set the directory where seed-tools is located

env >> "$SEED_TOOLS_DIR/debug.log"
echo "Arguments: $@" >> "$SEED_TOOLS_DIR/debug.log"

ARGS=("$@")
CLEAN_ARGS=()
for ARG in "${ARGS[@]}"; do
    if [ -n "$ARG" ]; then
        CLEAN_ARGS+=("$ARG")
    fi
done

if [[ "${CLEAN_ARGS[-1]}" =~ ^[0-9]{4}$ ]]; then
    CUSTOM_CAT_TYPE="${CLEAN_ARGS[-1]}"
    unset CLEAN_ARGS[-1]
    CLEAN_ARGS+=("-c" "$CUSTOM_CAT_TYPE")
fi

cd "$SEED_TOOLS_DIR" || exit 1
./seed-tools "${CLEAN_ARGS[@]}" >> "$SEED_TOOLS_DIR/debug.log" 2>&1