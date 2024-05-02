# RAP -- Rust Analysis Platform
RAP is a static Rust analysis platform developped by researchers at Artisan-Lab, Fudan University. The projects aims to provide a basis for Rust programers to develope or use fancy static analysis features beyond the rustc compiler. Currently, we have implemented two features:

- **SafeDrop**: a feature to detect use-after-free/double-free and other dangling pointer issues incurred by unsafe code.

- **rCanary**: for memory leakage detection. 

The project is still under heavy development. For further details, please refer to the [RAP-Book](https://artisan-lab.github.io/RAP-Book).

## Quick Start

```shell
git clone https://github.com/Artisan-Lab/RAP.git
cd rap
git submodule update --init --recursive
./00-install-rap-rust.sh
./01-install-rap-cargo.sh
./02-install-rap-llvm.sh
```

## Usage

Enter your Rust project folder and execute the following command based on your needs.

```
cargo rap -- -SAFEDROP
cargo rap -- -RCANARY
```
