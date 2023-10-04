# RLC -- Rust Leakage Checker

This is the main source code repository for rlc.
It contains the source code of **rlc**, **cargo-rlc**, **rust-llvm-heap-analysis-tool (rlc_phase_llvm)** and the **rlc-constraint-solver**.

Note: this **README** is for _users_ rather than _contributors_. 

## Quick Start
### Installing from Source Code

The rlc build system uses a shell script called `install.sh` to build all components, which manages the compiling process.
It lives in the root of the rlc project.

`install_rlc.sh` script can be run directly on most **unix-like** systems, such as mac-os, linux, etc.

Note: before running `install_rlc.sh` script, you should change current dir to the root of rlc project.

```shell
./install_rlc.sh
```

### Building on a Unix-like system
1. Make sure you have installed the dependencies:

   * clang++ 14.0 or later
   * llvm 14.0 or later
   * python 3.9 or later
   * z3 4.10 or later
   * GNU make 3.81 or later 
   * cmake 3.24 or later 
   * git
   * rustup 1.25 or later
   * rustc/cargo nightly-2022-08-01
   * cargo components: rust-src, rustc-dev, llvm-tools-preview

2. Clone the source with git:

```shell
git clone https://github.com/vaynnecol/rlc.git
cd rlc
```

To be done: not upload yet

3. Configure the build settings:

The configuration of rlc building system can be modified in file `Cargo.toml` and `install_rlc.sh`.
The Rust build system uses a file named `Cargo.toml` in the root of the source tree to determine various configuration settings for the build.
Specifically, `Cargo.toml` can option the compilation of **rlc** and **cargo-rlc**.

`install_rlc.sh` can option the compilation of **rust-llvm-heap-analysis-tool**. The binary of this tool named as **rlc_phase_llvm** will be automated added to your system environment in this script.

```shell
#for debug version
cargo install --debug --path "$(dirname "$0")" --force --features backtraces
#for release version
cargo install --path "$(dirname "$0")" --force
```

If you use bash instead of zsh, you should change in `install.sh`:
```shell
#for zsh
echo $p >> ~/.zshrc
#for bash
echo $p >> ~/.bashrc 
```

4. Build and install:
```shell
./install_rlc.sh
```

When complete, `./install_rlc.sh` install will place several programs into `$PREFIX/bin`: `rlc`, the Rust Leakage Checker; `cargo-rlc`, the specific tool embedded in cargo to call `rlc`; `rlc_phase_llvm`, the tool to scan llvm-ir for rust crate and check the usage of heap resource; `rlc_solver`, the constraint solver for whole `rlc` system.

### Building on Windows

There are two prominent ABIs in use on Windows: the native `MSVC` ABI used by Visual Studio, and the `GNU` ABI used by the GCC toolchain.

Currently, the rlc only supports for interop with `GNU` software built using the `MinGW/MSYS2` toolchain use the `GNU` build.

`MSYS2` can be used to easily build Rust on Windows:

1. Grab the latest `MSYS2` installer and go through the installer.

2. Run `mingw64_shell.bat` from wherever you installed `MSYS2 (i.e. C:\msys64)`. (As of the latest version of `MSYS2` you have to run `msys2_shell.cmd -mingw64` from the command line instead)

3. From this terminal, install the required tools:

```shell
# Update package mirrors (may be needed if you have a fresh install of MSYS2)
pacman -Sy pacman-mirrors

# Install build tools needed for Rust. If you're building a 32-bit compiler,
# then replace "x86_64" below with "i686". If you've already got git, python,
# or CMake installed and in PATH you can remove them from this list. Note
# that it is important that you do **not** use the 'python2', 'cmake' and 'ninja'
# packages from the 'msys2' subsystem. The build has historically been known
# to fail with these packages.
pacman -S git \
            make \
            diffutils \
            tar \
            mingw-w64-x86_64-python \
            mingw-w64-x86_64-cmake \
            mingw-w64-clang-x86_64-toolchain
```

4. Navigate to rlc source code (or clone it), then build it.

Note: we do not advice the user to use the windows as host platform.

## Using RLC
... to be done

including the optional arguments for rlc, the emitter dir and introduction, and the logging-output system (unfinished yet)