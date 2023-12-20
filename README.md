# RAP -- Rust Analysis Platform
This is the main source code repository for Rust Analysis Platform.
RAP contains the source code of `rap-rust`, `rap-llvm`, `rap-Z3`, `librap`, `rap`, and `cargo-rap`.
Note: **README** is for _users_ rather than _contributors_.
## Quick Start
### Installing from Source Code
RAP have two major modules: `rap` and `rap-rust` _forked from `rust` master branch_. They should be compiled followed by our instructions.

### Building on a Unix-like system (Linux / Macintosh)
#### 1. Make sure you have installed the dependencies:
* `git`
* `ninja`
* `clang++` 17.0 or later
* `llvm` 17.0 or later
* `python3` 3.11 or later
* `z3` 4.12 or later
* `make` 3.81 or later
* `cmake` 3.27 or later
* `rustup` 1.26 or later

**~~We do not need any version of `rustc` or `cargo`~~, we will bootstrap a modified `rustc` toolchain  for further use.**

#### 2. Clone the source with `git`:

```shell
git clone https://github.com/Artisan-Lab/RAP.git
cd rap
git submodule update --init --recursive
```

#### 3. Build and install `rap-rust`

`rap-rust` is forking from the original branch of `rust`. We modified the source code to perform self-defined static
analysis. It must be compiled as dependencies before building `rap-cargo`.

Now we need to bootstrap `rustc` to `stage2`. As all we need is `libstd*` and `librustc_*`, those artifacts are from
`stage2`, therefore the compiler needs to be bootstrapped to `stage2` to generate them.

```shell
# The script can be run directly on most unix-like systems, such as Macintosh, Linux, etc.
./00-install-rap-rust.sh
```

It performs the following phases:
- PHASE1: Checking operating system
- PHASE2: Checking build dependencies `rustup`
-  PHASE3: Building, installing and linking `rap-rust` into `cargo`
    ```shell
    # Copy config.toml to rap-rust
    cp -f ./config.toml ./rust/config.toml

    # Start Bootstrap
    # Using comiler/rustc due to needing rustc_*.rlib/.so
    cd rust && ./x.py build compiler/rustc -i --stage 2

    # Link rap-rust toolchain to current rustup and cargo
    rustup toolchain link rap-rust build/${HOST_TRIPLE}/stage2
    ```

#### 4. Build and install `rap-cargo`:
Configurations of RAP building system can be modified in `Cargo.toml` and `01-install-rap-cargo.sh`. The build system uses a file named `Cargo.toml` in the root of the source tree to determine various configuration settings. `Cargo.toml` can option the compilation of `rap` and `rap-cargo`. 

```shell
# The script can be run directly on most unix-like systems, such as Macintosh, Linux, etc.
./01-install-rap-cargo.sh
```

It performs the following phases:
- PHASE1: Checking operating system
- PHASE2: Checking working directory for `rap`
- PHASE3: Checking link of `rap-rust`
- PHASE4: Building, installing and linking `rap` into `cargo`

    It will install the bin `rap` into `cargo` components first:
    ```shell
    # Execution self cleanup procedure
    cd rap && cargo clean

    # Build and install binary 'rap' into cargo components
    # For debug version of `rap`
    # cargo install --debug --path "$(dirname "$0")" --force --features backtraces
    # For release version of `rap`
    RUSTC_INSTALL_BINDIR=bin CFG_RELEASE_CHANNEL=nightly CFG_RELEASE=nightly cargo install --path "$(dirname "$0")" --force
    ```
    The environmental variables will be catched by srcipt automatically, including `${RAP_DIR}` and `${HOST_TRIPLE}`.
    ```shell
    # Link to .rlib / .rmeta / .so files; for Linux
    export LD_LIBRARY_PATH="${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib:$LD_LIBRARY_PATH"
    export LD_LIBRARY_PATH="${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib/rustlib/${HOST_TRIPLE}/lib:$LD_LIBRARY_PATH"

    # Link to .rlib / .rmeta / .dylib files; for Macintosh
    export DYLD_LIBRARY_PATH="${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib:$DYLD_LIBRARY_PATH"
    export DYLD_LIBRARY_PATH="${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib/rustlib/${HOST_TRIPLE}/lib:$DYLD_LIBRARY_PATH"

    # Link libraries searching paths for rustc, by using RUSTFLAGs -L DIR
    export RUSTFLAGS="-L ${RAP_DIR}/rust/build/${HOST_TRIPLE}/stage2/lib"
    ``` 
When complete, `01-install-rap-cargo.sh` will link several programs into `$PREFIX/bin`: `rap`, the `rustc` wrapper program for Rust Analysis Platform; `rap-cargo`, the subcomponent in `cargo` to invoke `rap`.
#### 5. Build and install rap-llvm
`02-install-rap-llvm.sh` can also option the compilation of `rap-llvm`. This binary will be compiled and automated added to your system environment.

```shell
# The script can be run directly on most unix-like systems, such as Macintosh, Linux, etc.
./02-install-rap-llvm.sh
```

It performs the following phases:
- PHASE1: Building `rap-llvm` by `CMake` and add it to local bins
    ```shell
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
    ```
- PHASE2: Writing for user shell

When complete, `02-install-rap-llvm.sh` will link `rap-llvm` (the tool to scan llvm-ir for rust crate and check the usage of heap resource) into `$PREFIX/bin`. 

### Building on Windows
**Note: we highly do not advice the user to use the windows as host platform.**

There are two prominent ABIs in use on Windows: the native `MSVC ABI` used by Visual Studio, and the `GNU ABI` used by 
the GCC toolchain.
RAP only supports for interop with `GNU` software built using the `MinGW/MSYS2` toolchain use the `GNU` build.
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