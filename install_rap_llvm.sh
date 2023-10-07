#!/bin/zsh

# Build and install `rap-llvm` into user local bins
echo "\e[4;33mNow building rap-llvm.\e[0m"
echo "\e[4;32mPHASE5: Building \e[1;36mrap-llvm \e[4;32mby \e[1;36mCMake \e[4;32mand add it to local bins.\e[0m"
# Generate building directory
cd rap-llvm && mkdir build

# Build `rap-llvm`
export RAP_DIR=$(dirname "$(readlink -f "$0")")

cmake -DCMAKE_BUILD_TYPE=Debug \
      -DCMAKE_DEPENDS_USE_COMPILER=FALSE \
      -DCMAKE_INSTALL_PREFIX=${RAP_DIR}/build \
      -G "CodeBlocks - Unix Makefiles" \
      -B "${RAP_DIR}/rap-llvm/build"\
      -S "${RAP_DIR}/rap-llvm"

cmake --build "${RAP_DIR}/rap-llvm/build" \
      --target rap-llvm -v -- -j 9

echo "\e[0;32mPHASE6: Building and installing \e[1;36mrap-llvm \e[0;32mfinished.\e[0m"
#
## Write environment variables into usr system
#p="export PATH=\"\$PATH:${PWD}/\""
#
## For zsh
#echo $p >> ~/.zshrc
## For bash
#echo $p >> ~/.bashrc
#
#export PATH="$PATH:${PWD}"
#