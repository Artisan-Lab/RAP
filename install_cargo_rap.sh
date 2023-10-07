#!/bin/zsh

os_type=$(uname -s)

echo "\e[4;33mNow building cargo rap for your toolchain.\e[0m"
echo "\e[4;33mPHASE1: Checking operating system.\e[0m"
if [ "$os_type" = "Linux" ]; then
    echo "Detection success: running on \e[1;36mLinux (x86_64-unknown-linux-gnu)\e[0m."
    export HOST_TRIPLE="x86_64-unknown-linux-gnu"
elif [ "$os_type" = "Darwin" ]; then
    echo "Detection success: running on \e[1;36mMacintosh (x86_64-apple-darwin)\e[0m."
    export HOST_TRIPLE="x86_64-apple-darwin"
elif [ "$os_type" = "FreeBSD" ]; then
    echo "Detection success: running on \e[1;36mFreeBSD (x86_64-unknown-linux-gnu)\e[0m."
    export HOST_TRIPLE="x86_64-unknown-linux-gnu"
else
    echo "Detection failed: running unsupported operating system: $os_type."
    echo "\e[4mPress any key to exit...\e[0m"
    read -n 1
    exit 1
fi

echo "\e[4;33mPHASE2: Checking working directory for \e[1;36mrap\e[4;33m.\e[0m"
export RAP_DIR=$(dirname "$(readlink -f "$0")")
echo "Detection success: working directory is \e[1;36m${RAP_DIR}\e[0m."

echo "\e[4;33mPHASE3: Checking link of \e[1;36mrap-rust\e[4;33m.\e[0m"
RUSTUP_SHOW=$(rustup show)
if [ -z "$(ls ${RAP_DIR}/rust)" ]; then
    echo "Detection failed: directory of rap-rust is empty, please build and install \e[1;36mrap-rust\e[0m first."
    echo "\e[4mPress any key to exit...\e[0m"
    read -n 1
    exit 2
else
  if echo "${RUSTUP_SHOW}" | grep -q "rap-rust"; then
    echo "Detection success: \e[1;36mrap-rust\e[0m has been linked into \e[1;36mrustup\e[0m toolchain."
  else
    echo "Detection failed: cannot find \e[1;36mrap-rust\e[0m, please build and install \e[1;36mrap-rust\e[0m first."
    echo "\e[4mPress any key to exit...\e[0m"
    read -n 1
    exit 2
  fi
fi

echo "\e[4;31mPHASE4: Building, installing and linking \e[1;36mrap \e[1;31minto \e[1;36mcargo\e[1;31m.\e[0m"
# Execution self cleanup procedure
cd rap && cargo clean

# Build and install binary 'rap' into cargo components
# For debug version of `rap`
# cargo install --debug --path "$(dirname "$0")" --force --features backtraces
# For release version of `rap`
RUSTC_INSTALL_BINDIR=bin CFG_RELEASE_CHANNEL=nightly CFG_RELEASE=nightly cargo install --path "$(dirname "$0")" --force

# Link to .rlib / .rmeta / .so files; for Linux
export LD_LIBRARY_PATH=${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib:$LD_LIBRARY_PATH
export LD_LIBRARY_PATH=${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib/rustlib/${HOST_TRIPLE}/lib:$LD_LIBRARY_PATH

# Link to .rlib / .rmeta / .dylib files; for Macintosh
export DYLD_LIBRARY_PATH=${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib:$DYLD_LIBRARY_PATH
export DYLD_LIBRARY_PATH=${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib/rustlib/${HOST_TRIPLE}/lib:$DYLD_LIBRARY_PATH

# Link libraries searching paths for rustc, by using RUSTFLAGs -L DIR
export RUSTFLAGS="-L ${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib":$RUSTFLAGS
export RUSTFLAGS="-L ${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib/rustlib/${HOST_TRIPLE}/lib":$RUSTFLAGS

echo "\e[0;32mBuilding success: building, installing and linking \e[1;36mrap \e[0;32mfinished.\e[0m"
echo "\e[1;33mBuild and install all components successfully.\e[0m"
echo "\e[4mPress any key to exit...\e[0m"
read -n 1

exit 0