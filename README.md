# RAP -- Rust Analysis Platform

This is the main source code repository for Rust Analysis Platform.

RAP contains the source code of **rap-rust**, **librap**, **rap**, **cargo-rap**, **llvm-heap-analysis-tool
(rap_phase_llvm)** and the **Z3-constraint-solver**.

Note: this **README** is for _users_ rather than _contributors_.

## Quick Start
### Installing from Source Code

RAP have two major sub-modules: **rap** and **rust** (**rap-rust** forking from original master branch). They should be 
compiled properly
by following our instructions.

### Building on a Unix-like system (linux / macos)
1. Make sure you have installed the dependencies:
    * git
    * clang++ 17.0 or later
    * llvm 17.0 or later
    * python 3.11.0 or later
    * z3 4.12 or later
    * GNU make 3.81 or later
    * cmake 3.27 or later
    * rustup 1.26 or later
    * rust toolchain (rustc/cargo) nightly

2. Clone the source with git:

```shell
git clone https://github.com/Artisan-Lab/RAP.git
cd rap
git submodule update --init --recursive
```

3. Build rap-rust

**rap-rust** is forking from the original branch of rust, and we modified the source code to perform self-defined static
analysis. It should be compiled before building RAP as its dependencies.

Now we need to bootstrap rustc to stage2. As all we need is the libstd, libcore and librustc, those artifacts from 
are generated from stage1, therefore the compiler needs to be bootstrapped to stage2 to produce them.

```shell
# Copy config.toml to rap-rust
cp ./config.toml ./rust/

# Start Bootstrap
cd rust
./x.py build --stage 2
```

Link rap-rust toolchain to current rustup and cargo:

```shell
# x86_64-unknown-linux-gnu/x86_64-apple-darwin
rustup toolchain link stage1 build/<host-triple>/stage1
rustup toolchain link stage2 build/<host-triple>/stage2
```

4. Build and install RAP:

The RAP build system uses a shell script called `install.sh` to build all components, which manages the compiling process.
It lives in the root of the RAP crate.

`install.sh` script can be run directly on most **unix-like** systems, such as macos, linux, etc.

Note: before running `install.sh` script, you should change current dir to the root of RAP crate.

The configuration of RAP building system can be modified in file `Cargo.toml` and `install.sh`.
The build system uses a file named `Cargo.toml` in the root of the source tree to determine various configuration 
settings for the build. Specifically, `Cargo.toml` can option the compilation of **rap** and **cargo-rap**.

`install.sh` can option the compilation of **llvm-heap-analysis-tool**. The binary of this tool named 
as **rap_phase_llvm** will be automated added to your system environment in this script.

```shell
./install.sh
```

It will install the bin into cargo components
```shell
# install cargo rap
RUSTC_INSTALL_BINDIR=bin CFG_RELEASE_CHANNEL=nightly CFG_RELEASE=nightly cargo install --path "$(dirname "$0")" --force

# export
export LD_LIBRARY_PATH=/<Users Dir>/RAP/rust/build/x86_64-apple-darwin/stage1/lib/rustlib/<host-triple>/lib:$LD_LIBRARY_PATH
```

If you use bash instead of zsh, you should change in `install.sh`:

```shell
#for zsh
echo $p >> ~/.zshrc
#for bash
echo $p >> ~/.bashrc 
```

When complete, `install.sh` install will place several programs into `$PREFIX/bin`: `rap`, the Rust Analysis 
Platform; `cargo-rap`, the specific tool embedded in cargo to call `rap`; `rap_phase_llvm`, the tool to scan llvm-ir 
for rust crate and check the usage of heap resource.

### Building on Windows

There are two prominent ABIs in use on Windows: the native `MSVC` ABI used by Visual Studio, and the `GNU` ABI used by 
the GCC toolchain.

Currently, the RAP only supports for interop with `GNU` software built using the `MinGW/MSYS2` toolchain use the `GNU` build.

`MSYS2` can be used to easily build Rust on Windows:

1. Download the latest `MSYS2` installer and go through the installer.
2. Run `mingw64_shell.bat` from wherever you installed `MSYS2 (i.e. C:\msys64)`. (As of the latest version of `MSYS2`
you have to run `msys2_shell.cmd -mingw64` from the command line instead)
3. From this terminal, install the required tools:

```shell
# Update package mirrors (may be needed if you have a fresh install of MSYS2)
pacman -Sy pacman-mirrors

# Install build tools needed for Rust. If you're building a 32-bit compiler,
# then replace "x86_64" below with "i686". If you've already got Git, Python,
# or CMake installed and in PATH you can remove them from this list.
# Note that it is important that you do **not** use the 'python2', 'cmake',
# and 'ninja' packages from the 'msys2' subsystem.
# The build has historically been known to fail with these packages.
pacman -S git \
            make \
            diffutils \
            tar \
            mingw-w64-x86_64-python \
            mingw-w64-x86_64-cmake \
            mingw-w64-x86_64-gcc \
            mingw-w64-x86_64-ninja
```

4. Navigate to RAP source code (or clone it), then build it.

Note: we do not advice the user to use the windows as host platform.