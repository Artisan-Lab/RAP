# RAP -- Rust Analysis Platform
RAP is a static Rust analysis platform developped by researchers at Artisan-Lab, Fudan University. The projects aims to provide a basis for Rust programers to develope or use fancy static analysis features beyond the rustc compiler. For further details, please refer to the [RAP-Book](https://artisan-lab.github.io/RAP-Book).

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

Enter your Rust project folder and execute the following command based on your needs.

### Use-After-Free Detection
Detect bugs like use-after-free and double free in Rust crates caused by unsafe code.
```
cargo rap -uaf
```

The feature is based on our SafeDrop paper published in TOSEM.  
```
@article{cui2023safedrop,
  title={SafeDrop: Detecting memory deallocation bugs of rust programs via static data-flow analysis},
  author={Cui, Mohan and Chen, Chengjun and Xu, Hui and Zhou, Yangfan},
  journal={ACM Transactions on Software Engineering and Methodology},
  volume={32},
  number={4},
  pages={1--21},
  year={2023},
  publisher={ACM New York, NY, USA}
}
```

### Memory Leakage Detection 
```
cargo rap -mleak
```

The feature is based on our rCanary paper published in TSE.  
```
@article{cui2023safedrop,
  title={rCanary: Detecting memory leaks across semi-automated memory management boundary in Rust},
  author={Cui, Mohan and Xu, Hui and Tian, Hongliang and and Zhou, Yangfan},
  journal={IEEE Transactions on Software Engineering},
  year={2024}
}
```
