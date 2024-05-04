RED='\033[4;31m'
YELLOW='\033[4;33m'
GREEN='\033[0;32m'
BLUE='\033[36m'
WHITE='\033[4m'
NC='\033[0m'

printf "%bNow building cargo rap for your toolchain.%b\n" "${YELLOW}" "${NC}"
printf "%bPHASE1: Checking operating system.%b\n" "${YELLOW}" "${NC}"
os_type=$(uname -s)
arch_type=$(uname -m)

if [ "$os_type" = "Linux" ]; then
    printf "Detection success: running on %bLinux (x86_64-unknown-linux-gnu)%b.\n" "${BLUE}" "${NC}"
    export HOST_TRIPLE="x86_64-unknown-linux-gnu"
elif [ "$os_type" = "Darwin" ] && [ "$arch_type" = "x86_64" ]; then
    printf "Detection success: running on %bMacintosh-Intel (x86_64-apple-darwin)%b.\n" "${BLUE}" "${NC}"
    export HOST_TRIPLE="x86_64-apple-darwin"
elif [ "$os_type" = "Darwin" ] && [ "$arch_type" = "arm64" ]; then
    printf "Detection success: running on %bMacintosh-Apple-Silicon (aarch64-apple-darwin)%b.\n" "${BLUE}" "${NC}"
    export HOST_TRIPLE="aarch64-apple-darwin"
elif [ "$os_type" = "FreeBSD" ]; then
    printf "Detection success: running on %bFreeBSD (x86_64-unknown-linux-gnu)%b.\n" "${BLUE}" "${NC}"
    export HOST_TRIPLE="x86_64-unknown-linux-gnu"
else
    printf "Detection failed: running unsupported operating system: $os_type.\n"
    printf "%bPress any key to exit...%b\n" "${WHITE}" "${NC}"
    read -n 1
    exit 1
fi

printf "%bPHASE2: Checking build dependencies %brustup%b.%b\n" "${YELLOW}" "${BLUE}" "${YELLOW}" "${NC}"
export RUSTUP_SHOW=$(rustup show)
export RAP_DIR=$(dirname "$(readlink -f "$0")")
if [ -z "$(ls ${RAP_DIR}/rust)" ]; then
    printf "Detection failed: directory of rap-rust is empty, please update submodule first.\n"
    printf "%bPress any key to exit...%b\n" "${WHITE}" "${NC}"
    read -n 1
    exit 2
else
  if echo "${RUSTUP_SHOW}" | grep -q "rustup"; then
    printf "Detection success: %brustup%b has been linked into %brustup%b toolchain.\n" "${BLUE}" "${NC}" "${BLUE}" "${NC}"
  else
    printf "Detection failed: cannot find %brustup%b, please install %brustup%b first.\n" "${BLUE}" "${NC}" "${BLUE}" "${NC}"
    printf "%bPress any key to exit...%b\n" "${WHITE}" "${NC}"
    read -n 1
    exit 2
  fi
fi

printf "%bPHASE3: Building, installing and linking %brap-rust %binto %bcargo%b.%b\n" "${RED}" "${BLUE}" "${RED}" "${BLUE}" "${RED}" "${NC}"
cp -f ./config.toml ./rust/config.toml
cd rust && ./x.py build compiler/rustc -i --stage 2
rustup toolchain link rap-rust build/${HOST_TRIPLE}/stage2

printf "%bBuilding success: building, installing and linking %brap-rust %bfinished.%b\n" "${GREEN}" "${BLUE}" "${GREEN}" "${NC}"
printf "%bBuild and install all components successfully.%b\n" "${YELLOW}" "${NC}"
printf "%bPress any key to exit...%b\n" "${WHITE}" "${NC}"
read -n 1
