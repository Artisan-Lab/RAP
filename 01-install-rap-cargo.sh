RED='\033[4;31m'
YELLOW='\033[4;33m'
GREEN='\033[0;32m'
BLUE='\033[36m'
WHITE='\033[4m'
NC='\033[0m'

printf "%bNow building cargo rap for your toolchain.%b\n" "${YELLOW}" "${NC}"
printf "%bPHASE1: Checking operating system.%b\n" "${YELLOW}" "${NC}"
os_type=$(uname -s)

if [ "$os_type" = "Linux" ]; then
    printf "Detection success: running on %bLinux (x86_64-unknown-linux-gnu)%b.\n" "${BLUE}" "${NC}"
    export HOST_TRIPLE="x86_64-unknown-linux-gnu"
elif [ "$os_type" = "Darwin" ]; then
    printf "Detection success: running on %bMacintosh (x86_64-apple-darwin)%b.\n" "${BLUE}" "${NC}"
    export HOST_TRIPLE="x86_64-apple-darwin"
elif [ "$os_type" = "FreeBSD" ]; then
    printf "Detection success: running on %bFreeBSD (x86_64-unknown-linux-gnu)%b.\n" "${BLUE}" "${NC}"
    export HOST_TRIPLE="x86_64-unknown-linux-gnu"
else
    printf "Detection failed: running unsupported operating system: $os_type.\n"
    printf "%bPress any key to exit...%b\n" "${WHITE}" "${NC}"
    read -n 1
    exit 1
fi

printf "%bPHASE2: Checking working directory for %brap%b.%b\n" "${YELLOW}" "${BLUE}" "${YELLOW}" "${NC}"
export RAP_DIR=$(dirname "$(readlink -f "$0")")
printf "Detection success: working directory is %b${RAP_DIR}%b.\n" "${BLUE}" "${NC}"

printf "%bPHASE3: Checking link of %brap-rust%b.%b\n" "${YELLOW}" "${BLUE}" "${YELLOW}" "${NC}"
RUSTUP_SHOW=$(rustup show)
if [ -z "$(ls ${RAP_DIR}/rust)" ]; then
    printf "Detection failed: directory of %brap-rust%b is empty, please build and install %brap-rust%b first.\n" "${BLUE}" "${NC}" "${BLUE}" "${NC}"
    printf "%bPress any key to exit...%b\n" "${WHITE}" "${NC}"
    read -n 1
    exit 2
else
  if echo "${RUSTUP_SHOW}" | grep -q "rap-rust"; then
    printf "Detection success: %brap-rust%b has been linked into %brustup%b toolchain.\n" "${BLUE}" "${NC}" "${BLUE}" "${NC}"
  else
    printf "Detection failed: cannot find %brap-rust%b, please build and install %brap-rust%b first.\n" "${BLUE}" "${NC}" "${BLUE}" "${NC}"
    printf "%bPress any key to exit...%b\n" "${WHITE}" "${NC}"
    read -n 1
    exit 2
  fi
fi

printf "%bPHASE4: Building, installing and linking %brap%b into %bcargo%b.%b\n" "${RED}" "${BLUE}" "${RED}" "${BLUE}" "${RED}" "${NC}"
# Execution self cleanup procedure
cd rap && cargo clean

# Build and install binary 'rap' into cargo components
# For debug version of `rap`
# cargo install --debug --path "$(dirname "$0")" --force --features backtraces
# For release version of `rap`
RUSTC_INSTALL_BINDIR=bin CFG_RELEASE_CHANNEL=nightly CFG_RELEASE=nightly cargo install --path "$(dirname "$0")" --force

# Link to .rlib / .rmeta / .so files; for Linux
export LD_LIBRARY_PATH="${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib:$LD_LIBRARY_PATH"
export LD_LIBRARY_PATH="${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib/rustlib/${HOST_TRIPLE}/lib:$LD_LIBRARY_PATH"

# Link to .rlib / .rmeta / .dylib files; for Macintosh
export DYLD_LIBRARY_PATH="${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib:$DYLD_LIBRARY_PATH"
export DYLD_LIBRARY_PATH="${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib/rustlib/${HOST_TRIPLE}/lib:$DYLD_LIBRARY_PATH"

# Link libraries searching paths for rustc, by using RUSTFLAGs -L DIR
export RUSTFLAGS="-L ${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib"

printf "%bBuilding success: building, installing and linking %brap %bfinished.%b\n" "${GREEN}" "${BLUE}" "${GREEN}" "${NC}"

# Write environment variables into usr system
printf "%bPHASE5: Writing for user shell.%b\n" "${RED}" "${NC}"
if echo "$SHELL" | grep -q "zsh"; then
    printf "Detection success: running on %bZsh%b.\n" "${BLUE}" "${NC}"
    export SHELL_CONFIG=~/.zshrc
elif echo "$SHELL" | grep -q "bash"; then
    printf "Detection success: running on %bBash%b.\n" "${BLUE}" "${NC}"
    export SHELL_CONFIG=~/.bashrc
else
    printf "Detection failed: please modify the config for: $SHELL."
    printf "%bPress any key to exit...%b\n" "${WHITE}" "${NC}"
    read -n 1
    exit 1
fi

if ! grep -q "LD_LIBRARY_PATH" ${SHELL_CONFIG}; then
    echo "export LD_LIBRARY_PATH=\"${LD_LIBRARY_PATH}\"" >> ${SHELL_CONFIG}
    echo "export DYLD_LIBRARY_PATH=\"${DYLD_LIBRARY_PATH}\"" >> ${SHELL_CONFIG}
    echo "export RUSTFLAGS=\"${RUSTFLAGS}\"" >> ${SHELL_CONFIG}
    printf "%bEnvironment variable has been successfully writen to ${SHELL_CONFIG}.%b\n" "${GREEN}" "${NC}"
else
    printf "Environment variable already exists in %b${SHELL_CONFIG}%b.\n" "${BLUE}" "${NC}"
fi

printf "%bBuild and install all components successfully.%b\n" "${GREEN}" "${NC}"
printf "%bPress any key to exit...%b\n" "${WHITE}" "${NC}"
read -n 1

exit 0