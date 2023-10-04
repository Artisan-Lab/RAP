#!/bin/zsh

echo "Building RAP and install RAP by Cargo"
#cargo clean
#for debug version
#cargo install --debug --path "$(dirname "$0")" --force --features backtraces
#for release version
#RUSTC_INSTALL_BINDIR=/home/vaynnecol/workspace/rust/build/x86_64-unknown-linux-gnu/stage1
RUSTC_INSTALL_BINDIR=bin CFG_RELEASE_CHANNEL=nightly CFG_RELEASE=nightly cargo install --path "$(dirname "$0")" --force
#export LD_LIBRARY_PATH=/Users/vaynnecol/WorkSpace/RAP/rust/build/dist/rust-std-1.75.0-dev-x86_64-apple-darwin/rust-std-x86_64-apple-darwin/lib/rustlib/x86_64-apple-darwin/lib:$LD_LIBRARY_PATH
#export LD_LIBRARY_PATH=/Users/vaynnecol/WorkSpace/RAP/rust/build/dist/rustc-dev-1.75.0-dev-x86_64-apple-darwin/rustc-dev/lib/rustlib/x86_64-apple-darwin/lib:$LD_LIBRARY_PATH
#export LD_LIBRARY_PATH=/Users/vaynnecol/WorkSpace/RAP/rust/build/x86_64-apple-darwin/stage1/lib/rustlib/x86_64-apple-darwin/lib:$LD_LIBRARY_PATH

echo "Building RAP_Phase_LLVM by CMake and add the tool into Environment"
cd phase_llvm || exit
mkdir "cmake-build"
cd cmake-build || exit
cmake -DCMAKE_BUILD_TYPE=Debug -DCMAKE_DEPENDS_USE_COMPILER=FALSE -G "CodeBlocks - Unix Makefiles" "../../$(dirname "$0")"
cmake --build "$(dirname "$0")" --target rap_phase_llvm -v -- -j 9
#p="export PATH=\"\$PATH:${PWD}/\""
#echo $p >> ~/.zshrc
#echo $p >> ~/.bashrc
export PATH="$PATH:${PWD}"