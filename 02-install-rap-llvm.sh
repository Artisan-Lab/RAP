RED='\033[4;31m'
YELLOW='\033[4;33m'
GREEN='\033[0;32m'
BLUE='\033[36m'
WHITE='\033[4m'
NC='\033[0m'

# Build and install `rap-llvm` into user local bins
printf "%bNow building rap-llvm.%b\n" "${YELLOW}" "${NC}"
printf "%bPHASE1: Building %brap-llvm%b by %bCMake%b and add it to local bins.%b\n" "${GREEN}" "${BLUE}" "${GREEN}" "${BLUE}" "${GREEN}" "${NC}"
# Generate building directory
cd rap-llvm && mkdir build

# Build `rap-llvm`
export RAP_DIR=$(dirname "$(readlink -f "$0")")

cmake -DCMAKE_BUILD_TYPE=Debug \
      -DCMAKE_DEPENDS_USE_COMPILER=FALSE \
      -DCMAKE_INSTALL_PREFIX=${RAP_DIR}/build \
      -G "CodeBlocks - Unix Makefiles" \
      -B "${RAP_DIR}/build"\
      -S "${RAP_DIR}"

cmake --build "${RAP_DIR}/build" \
      --target rap-llvm -v -- -j 9

printf "%bBuilding success: building %brap-llvm%b finished.%b\n" "${GREEN}" "${BLUE}" "${GREEN}" "${NC}"

printf "%bPHASE2: Writing for user shell.%b\n" "${RED}" "${NC}"
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

if ! grep -q "rap-llvm" ${SHELL_CONFIG}; then
    export PATH="$PATH:${PWD}/build"
    echo "export PATH=\"${PATH}\"" >> ${SHELL_CONFIG}
    printf "%bBinary has been successfully writen to ${SHELL_CONFIG}.%b\n" "${GREEN}" "${NC}"
else
    printf "Environment variable already exists in %b${SHELL_CONFIG}%b.\n" "${BLUE}" "${NC}"
fi

printf "%bBuild and install all components successfully.%b\n" "${GREEN}" "${NC}"
printf "%bPress any key to exit...%b\n" "${WHITE}" "${NC}"
read -n 1

exit 0