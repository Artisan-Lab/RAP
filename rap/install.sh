#!/bin/zsh

echo "Building RAP and install RAP by Cargo"

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

# Build and install `rap-llvm` into user local bins
echo "Building RAP_Phase_LLVM by CMake and add the tool into Environment"

# Generate building directory
cd phase_llvm || exit
mkdir "cmake-build"

# Build `rap-llvm`
cd cmake-build || exit
cmake -DCMAKE_BUILD_TYPE=Debug -DCMAKE_DEPENDS_USE_COMPILER=FALSE -G "CodeBlocks - Unix Makefiles" "../../$(dirname "$0")"
cmake --build "$(dirname "$0")" --target rap_phase_llvm -v -- -j 9


# Write environment variables into usr system
p="export PATH=\"\$PATH:${PWD}/\""

# For zsh
echo $p >> ~/.zshrc
# For bash
echo $p >> ~/.bashrc

export PATH="$PATH:${PWD}"