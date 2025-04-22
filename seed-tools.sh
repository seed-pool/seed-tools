#!/bin/bash
USER_DIR="/home/user" # Replace with the actual user directory

env >> "$USER_DIR/seed-tools/debug.log"
echo "Arguments: $@" >> "$USER_DIR/seed-tools/debug.log"

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

cd "$USER_DIR/seed-tools" || exit 1
./seed-tools "${CLEAN_ARGS[@]}" >> "$USER_DIR/seed-tools/debug.log" 2>&1