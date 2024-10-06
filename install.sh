# cd rap && cargo clean
# cargo install --path . 

#!/bin/bash

# Define color variables
BLUE='\033[0;34m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

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
LIBRARY_PATH="$HOME/.rustup/toolchains/nightly-2023-10-05-x86_64-unknown-linux-gnu/lib"
os_type=$(uname -s)

if ! grep -q "LD_LIBRARY_PATH.*$LIBRARY_PATH" "$SHELL_CONFIG"; then
    if [ "$os_type" = "Linux" ]; then
    	export LD_LIBRARY_PATH="$LIBRARY_PATH:$LD_LIBRARY_PATH"
        if grep -q "LD_LIBRARY_PATH" "$SHELL_CONFIG"; then
            sed -i '/LD_LIBRARY_PATH/d' "$SHELL_CONFIG"
            printf "%bOld LD_LIBRARY_PATH definition has been removed from %s.%b\n" "${GREEN}" "$SHELL_CONFIG" "${NC}"
        fi
        echo "export LD_LIBRARY_PATH=\"${LD_LIBRARY_PATH}\"" >> "$SHELL_CONFIG"
        printf "%bEnvironment variables have been successfully written to %s.%b\n" "${GREEN}" "$SHELL_CONFIG" "${NC}"
    elif [ "$os_type" = "Darwin" ]; then
        export DYLD_LIBRARY_PATH="$LIBRARY_PATH:$DYLD_LIBRARY_PATH"
        if grep -q "DYLD_LIBRARY_PATH" "$SHELL_CONFIG"; then
            sed -i '/DYLD_LIBRARY_PATH/d' "$SHELL_CONFIG"
            printf "%bOld DYLD_LIBRARY_PATH definition has been removed from %s.%b\n" "${GREEN}" "$SHELL_CONFIG" "${NC}"
        fi
        echo "export DYLD_LIBRARY_PATH=\"${DYLD_LIBRARY_PATH}\"" >> "$SHELL_CONFIG"
        printf "%bEnvironment variables have been successfully written to %s.%b\n" "${GREEN}" "$SHELL_CONFIG" "${NC}"
    fi
else
    printf "%bLibrary path already exists in %s.%b\n" "${BLUE}" "$SHELL_CONFIG" "${NC}"
fi

printf "%bTo apply the changes, run: %bsource %s%b\n" "${GREEN}" "${BLUE}" "$SHELL_CONFIG" "${NC}"
