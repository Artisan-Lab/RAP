# RAP -- Rust Analysis Platform
RAP is a static Rust analysis platform developped by researchers at [Artisan-Lab](https://hxuhack.github.io), Fudan University. The projects aims to provide a basis for Rust programers to develope or use fancy static analysis features beyond the rustc compiler. For further details, please refer to the [RAP-Book](https://artisan-lab.github.io/RAP-Book).

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

Enter your Rust project folder with a `Cargo.toml` file. If your project contains a `rust-toolchain.toml` file, we recommend disabling or removing it.
```shell
cargo rap -help
```

### Use-After-Free Detection
Detect bugs like use-after-free and double free in Rust crates caused by unsafe code.
```shell
cargo rap -uaf
```

If RAP gets stuck after executing `cargo clean`, try manually downloading metadata dependencies by running `cargo metadata`.

The feature is based on our SafeDrop paper published in TOSEM.  
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

The feature is based on our rCanary work which will appear in TSE
```
@article{cui2024rcanary,
  title={rCanary: rCanary: Detecting memory leaks across semi-automated memory management boundary in Rust},
  author={Mohan Cui, Hongliang Tian, Hui Xu, and Yangfan Zhou},
  journal={IEEE Transactions on Software Engineering},
  year={2024},

