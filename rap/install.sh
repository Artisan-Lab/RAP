#!/bin/zsh

echo -e "\e[1;31mBuilding and install \e[1;36mrap \e[1;31minto \e[1;36mcargo\e[1;31m.\e[0m"

# Execution self cleanup procedure
cargo clean

# Build and install binary 'rap' into cargo components
# For debug version of `rap`
# cargo install --debug --path "$(dirname "$0")" --force --features backtraces
# For release version of `rap`
RUSTC_INSTALL_BINDIR=bin CFG_RELEASE_CHANNEL=nightly CFG_RELEASE=nightly cargo install --path "$(dirname "$0")" --force

# Link to .rlib / .rmeta / .so files; for Linux
export LD_LIBRARY_PATH=/<working-dir>/RAP/rust/build/<host-triple>/stage2/lib:$LD_LIBRARY_PATH
export LD_LIBRARY_PATH=/<working-dir>/RAP/rust/build/<host-triple>/stage2/lib/rustlib/<host-triple>/lib:$LD_LIBRARY_PATH

# Link to .rlib / .rmeta / .dylib files; for Macintosh
export DYLD_LIBRARY_PATH=/<working-dir>/RAP/rust/build/<host-triple>/stage2/lib:$DYLD_LIBRARY_PATH
export DYLD_LIBRARY_PATH=/<working-dir>/RAP/rust/build/<host-triple>/stage2/lib/rustlib/<host-triple>/lib:$DYLD_LIBRARY_PATH

# Link libraries searching paths for rustc, by using RUSTFLAGs -L DIR
export RUSTFLAGS="-L /<working-dir>/RAP/rust/build/<host-triple>/stage2/lib":$RUSTFLAGS
export RUSTFLAGS="-L /<working-dir>/RAP/rust/build/<host-triple>/stage2/lib/rustlib/<host-triple>/lib":$RUSTFLAGS

echo -e "\e[1;32mBuilding and installing \e[1;36mrap \e[1;32mfinished.\e[0m"

# Build and install `rap-llvm` into user local bins
echo -e "\e[1;32mBuilding \e[1;36mrap-llvm \e[1;32mby \e[1;36mCMake \e[1;32mand add to local bins.\e[0m"

# Generate building directory
cd phase_llvm || exit
mkdir "cmake-build"

# Build `rap-llvm`
cd cmake-build || exit
cmake -DCMAKE_BUILD_TYPE=Debug -DCMAKE_DEPENDS_USE_COMPILER=FALSE -G "CodeBlocks - Unix Makefiles" "../../$(dirname "$0")"
cmake --build "$(dirname "$0")" --target rap_phase_llvm -v -- -j 9

echo -e "\e[1;32mBuilding and installing \e[1;36mrap-llvm \e[1;32mfinished.\e[0m"

# Write environment variables into usr system
p="export PATH=\"\$PATH:${PWD}/\""

# For zsh
echo $p >> ~/.zshrc
# For bash
echo $p >> ~/.bashrc

export PATH="$PATH:${PWD}"

echo -e "\e[1;33mBuild and install all components successfully.\e[0m"
echo -e "\e[1;33mPress any key to exit...\e[0m"
read -n 1