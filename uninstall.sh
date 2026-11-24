#!/usr/bin/env bash
set -e

# Colors
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${CYAN}=== ⚡ Uninstalling SpaceSage ===${NC}"

TARGET_LOCATIONS=(
    "$HOME/.local/bin/spacesage"
    "/usr/local/bin/spacesage"
    "/usr/bin/spacesage"
    "$HOME/.cargo/bin/spacesage"
    "$HOME/.local/bin/arch-disk-tui"
    "/usr/local/bin/arch-disk-tui"
    "/usr/bin/arch-disk-tui"
    "$HOME/.cargo/bin/arch-disk-tui"
)

FOUND=0

for loc in "${TARGET_LOCATIONS[@]}"; do
    if [ -f "$loc" ]; then
        if [ -w "$loc" ]; then
            rm -f "$loc"
            echo -e "${GREEN}✓ Removed: $loc${NC}"
            FOUND=1
        else
            echo -e "${YELLOW}Need sudo permissions to remove: $loc${NC}"
            sudo rm -f "$loc"
            echo -e "${GREEN}✓ Removed (via sudo): $loc${NC}"
            FOUND=1
        fi
    fi
done

# Also check which spacesage or arch-disk-tui in PATH
for bname in "spacesage" "arch-disk-tui"; do
    WHICH_PATH=$(command -v "$bname" 2>/dev/null || true)
    if [ -n "$WHICH_PATH" ] && [ -f "$WHICH_PATH" ]; then
        if [ -w "$WHICH_PATH" ]; then
            rm -f "$WHICH_PATH"
            echo -e "${GREEN}✓ Removed: $WHICH_PATH${NC}"
            FOUND=1
        else
            echo -e "${YELLOW}Need sudo permissions to remove: $WHICH_PATH${NC}"
            sudo rm -f "$WHICH_PATH"
            echo -e "${GREEN}✓ Removed (via sudo): $WHICH_PATH${NC}"
            FOUND=1
        fi
    fi
done

if [ $FOUND -eq 1 ]; then
    echo -e "${GREEN}✓ SpaceSage has been completely uninstalled.${NC}"
else
    echo -e "${YELLOW}No installed SpaceSage binary found.${NC}"
fi
