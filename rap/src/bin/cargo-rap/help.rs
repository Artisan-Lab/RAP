pub const RAP_HELP: &str = r#"
Usage:
    cargo rap [rap options] -- [cargo check options]

Rap Options:

Use-After-Free/double free detection.
    -F or -uaf       command: "cargo rap -uaf"

Memory leakage detection.
    -M or -mleak     command: "cargo rap -mleak"

Unsafe code tracing
    -UI or -uig      generate unsafe code isolation graphs

Dataflow tracing
    -dataflow        generate dataflow graphs

General command: 
    -H or -help:     show help information
    -V or -version:  show the version of RAP

Debugging options:
    -mir             print the MIR of each function

NOTE: multiple detections can be processed in single run by 
appending the options to the arguments. Like `cargo rap -F -M`
will perform two kinds of detection in a row.

e.g.
1. detect use-after-free and memory leak for a riscv target:
   cargo rap -F -M -- --target riscv64gc-unknown-none-elf
2. detect use-after-free and memory leak for tests:
   cargo rap -F -M -- --tests
3. detect use-after-free and memory leak for all members:
   cargo rap -F -M -- --workspace
"#;

pub const RAP_VERSION: &str = r#"
rap version 0.1
released at 2024-07-23
developped by artisan-lab @ Fudan university 
"#;
