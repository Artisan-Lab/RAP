# RAP -- Rust Analysis Platform ![logo](logo.png)
RAP is a static Rust analysis platform developed by researchers at [Artisan-Lab](https://hxuhack.github.io), Fudan University. The project aims to provide a foundation for Rust programmers to develop or use advanced static analysis features beyond those offered by the rustc compiler. For further details, please refer to the [RAP-Book](https://artisan-lab.github.io/RAP-Book).

The project is still under heavy development. 

## Quick Start

```shell
git clone https://github.com/Artisan-Lab/RAP.git
cd rap
git submodule update --init --recursive
./0-install-rap-rust.sh
./1-install-rap-cargo.sh
```

## Usage

Navigate to your Rust project folder containing a `Cargo.toml` file. If your project includes a `rust-toolchain.toml` file, we recommend disabling or removing it.
```shell
cargo rap -help
```

If the command fails, try setting the default Rust toolchain to `rap-rust`.
```
rustup default rap-rust
```

### Use-After-Free Detection
Detect bugs such as use-after-free and double free in Rust crates caused by unsafe code.
```shell
cargo rap -uaf
```

If RAP gets stuck after executing `cargo clean`, try manually downloading metadata dependencies by running `cargo metadata`.

The feature is based on our SafeDrop paper, which was published in TOSEM.  
```
@article{cui2023safedrop,
  title={SafeDrop: Detecting memory deallocation bugs of rust programs via static data-flow analysis},
  author={Mohan Cui, Chengjun Chen, Hui Xu, and Yangfan Zhou},
  journal={ACM Transactions on Software Engineering and Methodology},
  volume={32},
  number={4},
  pages={1--21},
  year={2023},
  publisher={ACM New York, NY, USA}
}
```

### Memory Leakage Detection 
Detect memory leakage bugs caused by apis like [ManuallyDrop](https://doc.rust-lang.org/std/mem/struct.ManuallyDrop.html) and [into_raw()](https://doc.rust-lang.org/std/boxed/struct.Box.html#method.into_raw).

```shell
cargo rap -mleak
```

The feature is based on our rCanary work, which was published in TSE
```
@article{cui2024rcanary,
  title={rCanary: rCanary: Detecting memory leaks across semi-automated memory management boundary in Rust},
  author={Mohan Cui, Hongliang Tian, Hui Xu, and Yangfan Zhou},
  journal={IEEE Transactions on Software Engineering},
  year={2024},

