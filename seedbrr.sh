#!/bin/bash
SEEDBRR_DIR="/home/user/seedbrr" # Set the directory where seedbrr is located

env >> "$SEEDBRR_DIR/debug.log"
echo "Arguments: $@" >> "$SEEDBRR_DIR/debug.log"

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

cd "$SEEDBRR_DIR" || exit 1
./seedbrr "${CLEAN_ARGS[@]}" >> "$SEEDBRR_DIR/debug.log" 2>&1