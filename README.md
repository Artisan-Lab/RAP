# ![logo](rap_logo.png)
RAP is a static Rust analysis platform developed by researchers at [Artisan-Lab](https://hxuhack.github.io), Fudan University. The project aims to provide a foundation for Rust programmers to develop or use advanced static analysis features beyond those offered by the rustc compiler. For further details, please refer to the [RAP-Book](https://artisan-lab.github.io/RAP-Book).

The project is still under heavy development. 

## Quick Start

```shell
git clone https://github.com/Artisan-Lab/RAP.git
cd RAP 
./install.sh
```

## Usage

Install `nightly-2024-06-30` on which rap is compiled with. This just needs to do once on your machine. If the toolchain exists,
this will do nothing.

```shell
rustup toolchain install nightly-2024-06-30 --profile minimal --component rustc-dev,rust-src,llvm-tools-preview
```

Navigate to your Rust project folder containing a `Cargo.toml` file. Then run `cargo-rap` with [toolchain override shorthand syntax].

[toolchain override shorthand syntax]: https://rust-lang.github.io/rustup/overrides.html#toolchain-override-shorthand

```shell
cargo +nightly-2024-06-30 rap # ... rest of options of cargo-rap
```

Alternatively, you can switch to the pinned toolchain ahead of time:

```rust
rustup default nightly-2024-06-30
cargo rap # ... rest of options of cargo-rap
```

Check out supported options with `-help`:

```shell
cargo +nightly-2024-06-30 rap -help
```

### Use-After-Free Detection
Detect bugs such as use-after-free and double free in Rust crates caused by unsafe code.
```shell
cargo +nightly-2024-06-30 rap -uaf
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
cargo +nightly-2024-06-30 rap -mleak
```

The feature is based on our rCanary work, which was published in TSE
```
@article{cui2024rcanary,
  title={rCanary: rCanary: Detecting memory leaks across semi-automated memory management boundary in Rust},
  author={Mohan Cui, Hongliang Tian, Hui Xu, and Yangfan Zhou},
  journal={IEEE Transactions on Software Engineering},
  year={2024},

