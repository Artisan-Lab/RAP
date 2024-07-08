# RAP -- Rust Analysis Platform
RAP is a static Rust analysis platform developped by researchers at Artisan-Lab, Fudan University. The projects aims to provide a basis for Rust programers to develope or use fancy static analysis features beyond the rustc compiler. For further details, please refer to the [RAP-Book](https://artisan-lab.github.io/RAP-Book).

The project is still under heavy development. 

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

### Use-After-Free Detection
Detect bugs like use-after-free and double free in Rust crates caused by unsafe code.
```
cargo rap -uaf
```

The feature is based on our SafeDrop paper published in TOSEM.  
Cui, Mohan, Chengjun Chen, Hui Xu, and Yangfan Zhou. "SafeDrop: Detecting memory deallocation bugs of rust programs via static data-flow analysis." ACM Transactions on Software Engineering and Methodology 32, no. 4 (2023): 1-21

### Memory Leakage Detection 
```
cargo rap -mleak
```
