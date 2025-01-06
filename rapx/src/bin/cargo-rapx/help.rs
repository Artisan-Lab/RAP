pub const RAPX_HELP: &str = r#"
Usage:
    cargo rapx [rapx options] -- [cargo check options]

RAPx Options:

Use-After-Free/double free detection.
    -F or -uaf       command: "cargo rapx -uaf"

Memory leakage detection.
    -M or -mleak     command: "cargo rapx -mleak"

Debugging options:
    -mir             print the MIR of each function

General command: 
    -H or -help:     show help information
    -V or -version:  show the version of RAPx

The following features are under development
Unsafe code tracing
    -UI or -uig      generate unsafe code isolation graphs

Controlflow tracing
    -callgraph       generate callgraphs

Dataflow tracing
    -dataflow        generate dataflow graphs

Automatic optimization
    -O or -opt       automatically detect code optimization chances

NOTE: multiple detections can be processed in single run by 
appending the options to the arguments. Like `cargo rapx -F -M`
will perform two kinds of detection in a row.

e.g.
1. detect use-after-free and memory leak for a riscv target:
   cargo rapx -F -M -- --target riscv64gc-unknown-none-elf
2. detect use-after-free and memory leak for tests:
   cargo rapx -F -M -- --tests
3. detect use-after-free and memory leak for all members:
   cargo rapx -F -M -- --workspace

Environment Variables (Values are case insensitive):
    RAP_LOG          verbosity of logging: trace, debug, info, warn
                     trace: print all the detailed RAP execution traces.
                     debug: display intermidiate analysis results.
                     warn: show bugs detected only.

    RAP_CLEAN        run cargo clean before check: true, false
                     * true is the default value except that false is set

    RAP_RECURSIVE    scope of packages to check: none, shallow, deep
                     * none or the variable not set: check for current folder
                     * shallow: check for current workpace members
                     * deep: check for all workspaces from current folder
                      
                     NOTE: for shallow or deep, rapx will enter each member
                     folder to do the check.
"#;

pub const RAPX_VERSION: &str = r#"
rapx version 0.1
released at 2025-01-06
developped by artisan-lab @ Fudan university 
"#;
