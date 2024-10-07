#!/bin/bash

# Define color variables
BLUE='\033[0;34m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

cd rap && cargo clean

case "$SHELL" in
    *zsh)
	printf "Detecting shell context success: running on %bZsh%b.\n" "${BLUE}" "${NC}"
	export SHELL_CONFIG=~/.zshrc
        ;;
    *bash)
	printf "Detecting shell context success: running on %bBash%b.\n" "${BLUE}" "${NC}"
	export SHELL_CONFIG=~/.bashrc
	;;
    *)
	printf "%bDetecting shell context failed.%b\n" "${RED}" "${NC}"
        exit 1
	;;
esac

# Set the library path to be added
#LIBRARY_PATH="$HOME/.rustup/toolchains/nightly-2023-10-05-x86_64-unknown-linux-gnu/lib"
toolchain_date="nightly-2023-10-05"
toolchain_file="rust-toolchain.toml"
if [ ! -f "$toolchain_file" ]; then
    printf "%bError: %s does not exist.%b\n" "${RED}" "$toolchain_file" "${NC}"
    exit 1
fi

os_type=$(uname -s)
arch_type=$(uname -m)

if [ "$os_type" = "Linux" ]; then
    # Update the channel field in rust-toolchain.toml
    toolchain="$toolchain_date-x86_64-unknown-linux-gnu"
    toolchain_lib="$HOME/.rustup/toolchains/$toolchain/lib"
    printf "%bDetected OS: Linux. Setting toolchain to %s%b\n" "${BLUE}" "$toolchain" "${NC}"
    sed -i.bak "s/^channel = \".*\"/channel = \"$toolchain\"/" "$toolchain_file"

    if ! grep -q "LD_LIBRARY_PATH.*$toolchain_lib" "$SHELL_CONFIG"; then
        if grep -q "LD_LIBRARY_PATH" "$SHELL_CONFIG"; then
    	    export LD_LIBRARY_PATH="$toolchain_lib:$LD_LIBRARY_PATH"
            sed -i '/LD_LIBRARY_PATH/d' "$SHELL_CONFIG"
            printf "%bOld LD_LIBRARY_PATH definition has been removed from %s.%b\n" "${GREEN}" "$SHELL_CONFIG" "${NC}"
        else 
    	    export LD_LIBRARY_PATH="$toolchain_lib"
        fi
        echo "export LD_LIBRARY_PATH=\"${LD_LIBRARY_PATH}\"" >> "$SHELL_CONFIG"
        printf "%bEnvironment variables have been successfully written to %s.%b\n" "${GREEN}" "$SHELL_CONFIG" "${NC}"
    fi
elif [ "$os_type" = "Darwin" ]; then
    if [ "$arch_type" = "x86_64" ]; then
        toolchain="$toolchain_date-x86_64-apple-darwin"
        toolchain_lib="$HOME/.rustup/toolchain/$toolchain/lib"
    else
        toolchain="$toolchain_date-aarch64-apple-darwin"
        toolchain_lib="$HOME/.rustup/toolchain/$toolchain/lib"
    fi
    printf "%bDetected OS: macOS. Setting toolchain to %s%b\n" "${BLUE}" "$toolchain" "${NC}"
    sed -i.bak "s/^channel = \".*\"/channel = \"$toolchain\"/" "$toolchain_file"
        
    if ! grep -q "DYLD_LIBRARY_PATH.*$toolchain_lib" "$SHELL_CONFIG"; then
        if grep -q "DYLD_LIBRARY_PATH" "$SHELL_CONFIG"; then
    	    export DYLD_LIBRARY_PATH="$toolchain_lib:$DYLD_LIBRARY_PATH"
            sed -i '/DYLD_LIBRARY_PATH/d' "$SHELL_CONFIG"
            printf "%bOld DYLD_LIBRARY_PATH definition has been removed from %s.%b\n" "${GREEN}" "$SHELL_CONFIG" "${NC}"
        else 
    	    export DYLD_LIBRARY_PATH="$toolchain_lib"
        fi
        echo "export DYLD_LIBRARY_PATH=\"${DYLD_LIBRARY_PATH}\"" >> "$SHELL_CONFIG"
        printf "%bEnvironment variables have been successfully written to %s.%b\n" "${GREEN}" "$SHELL_CONFIG" "${NC}"
    fi
fi

cargo install --path . 

printf "%bTo apply the changes, you may need to run: %bsource %s%b\n" "${GREEN}" "${BLUE}" "$SHELL_CONFIG" "${NC}"
