use rap::{RapPhase, rap_info,
          RAP_DEFAULT_ARGS, RAP_ROOT, RAP_LLVM_CACHE, RAP_LLVM_IR};
use rap::components::log::{Verbosity, rap_error_and_exit};
use rap::components::fs::{rap_create_dir, rap_remove_dir, rap_copy_file, rap_can_read_dir};

use std::env;
use std::iter::TakeWhile;
use std::process::Command;
use std::path::{PathBuf, Path};
use std::time::Duration;
use std::fmt::{Display, Formatter};
use std::process;

use rustc_version::VersionMeta;
use wait_timeout::ChildExt;
use walkdir::WalkDir;

const CARGO_RAP_HELP: &str = r#"Runs RAP to test and check Rust crates

Usage:
    cargo rap [<cargo options>...] [--] [<rustc/rap options>...]

Options:
Normal options:
    --help              Print help message
    -MIR=V              Set Verbose to print Rust MIR of each function
    -MIR==VV            Print more information than -MIR=V. Note that it will overwite -MIR=V.

SafeDrop:
    -SAFEDROP           Enable analysis in SafeDrop.

RCanary:
    -RCANARY            Enable analysis in RCanary, and it has following sub commands.
    -ADT=V              Print the pair of the result of type analysis, including the type definition and the analysis result.
    -Z3-GOAL=V          Emit the Z3 formula of the given function, it is in the SMT-Lib format.
    -GRAN=LOW           Default grain set for RCANAY, use MEDIUM/HIGH/ULTRA to overwrite.
    -ICX-SLICE=V        Set Verbose to print the middle metadate for RCANAY debug.

The cargo options are exactly the same as for `cargo run` and `cargo test`, respectively.

Examples:
    cargo rap run
"#;


fn show_help() {
    println!("{}", CARGO_RAP_HELP);
}

fn show_version() {
    let version = format!("rap {}", env!("CARGO_PKG_VERSION"));
    println!("The RAP version: {}", version);
}

// Determines whether a `--flag` is present.
fn has_arg_flag(name: &str) -> bool {
    // Stop searching at `--`.
    let mut args = env::args().take_while(|val| val != "--");
    args.any(|val| val == name)
}

fn has_rap_arg_flag(name: &str) -> bool {
    // Begin searching at `--`
    let mut args = env::args().skip_while(|val| val == "--");
    args.any(|val| val == name)
}

/// Yields all values of command line flag `name`.
struct ArgFlagValueIter<'a> {
    args: TakeWhile<env::Args, fn(&String) -> bool>,
    name: &'a str,
}

impl<'a> ArgFlagValueIter<'a> {
    fn new(name: &'a str) -> Self {
        Self {
            // Stop searching at `--`.
            args: env::args().take_while(|val| val != "--"),
            name,
        }
    }
}

impl Iterator for ArgFlagValueIter<'_> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let arg = self.args.next()?;
            if !arg.starts_with(self.name) {
                continue;
            }
            // Strip leading `name`.
            let suffix = &arg[self.name.len()..];
            if suffix.is_empty() {
                // This argument is exactly `name`; the next one is the value.
                return self.args.next();
            } else if suffix.starts_with('=') {
                // This argument is `name=value`; get the value.
                // Strip leading `=`.
                return Some(suffix[1..].to_owned());
            }
        }
    }
}

/// Gets the value of a `--flag`.
fn get_arg_flag_value(name: &str) -> Option<String> {
    ArgFlagValueIter::new(name).next()
}

/// Returns the path to the `rap` binary
fn find_rap() -> PathBuf {
    let mut path = env::current_exe()
        .expect("current executable path invalid");
    path.set_file_name("rap");
    path
}

fn rap() -> Command {
    Command::new(find_rap())
}

fn version_info() -> VersionMeta {
    VersionMeta::for_command(rap()).
        expect("failed to determine underlying rustc version of RAP")
}

fn test_sysroot_consistency() {
    fn get_sysroot(mut cmd: Command) -> PathBuf {
        let output = cmd
            .arg("--print")
            .arg("sysroot")
            .output()
            .expect("Failed to run rustc to get sysroot.");
        let stdout = String::from_utf8(output.stdout)
            .expect("Invalid UTF-8: stdout.");
        let stderr = String::from_utf8(output.stderr)
            .expect("Invalid UTF-8: stderr.");
        let stdout = stdout.trim();

        assert!(
            output.status.success(),
            "Termination unsuccessful when getting sysroot.\nstdout: {}\nstderr: {}",
            stdout,
            stderr,
        );

        PathBuf::from(stdout)
            .canonicalize()
            .unwrap_or_else(|_| panic!("Failed to canonicalize sysroot:{}", stdout))
    }

    let rustc_sysroot = get_sysroot(Command::new("rustc"));

    rap_info!("Detecting RUSTC SYSRROT: {:?}", rustc_sysroot);
    let rap_sysroot = get_sysroot(Command::new(find_rap()));
    rap_info!("Detecting RAP   SYSRROT: {:?}", rap_sysroot);

    assert_eq!(
        rustc_sysroot,
        rap_sysroot,
        "rap was built for a different sysroot than the rustc in your current toolchain.\n\
             Please use the same toolchain to run rap that you used to build it!\n\
             rustc sysroot: `{}`\nrap sysroot: `{}`",
        rustc_sysroot.display(),
        rap_sysroot.display()
    );
}

fn make_package() -> cargo_metadata::Package {
    // We need to get the manifest, and then the metadata, to enumerate targets.
    let manifest_path =
        get_arg_flag_value("--manifest-path")
            .map(|s| Path::new(&s).canonicalize().unwrap());

    let mut cmd = cargo_metadata::MetadataCommand::new();
    if let Some(manifest_path) = &manifest_path {
        cmd.manifest_path(manifest_path);
    };
    let mut metadata = match cmd.exec() {
        Ok(metadata) => metadata,
        Err(e) => rap_error_and_exit(format!("Cannot obtain Cargo metadata: {}", e)),
    };

    let current_dir = env::current_dir();
    let package_index = metadata
        .packages
        .iter()
        .position(|package| {
            let package_manifest_path = Path::new(&package.manifest_path);
            if let Some(manifest_path) = &manifest_path {
                package_manifest_path == manifest_path
            } else {
                let current_dir = current_dir
                    .as_ref()
                    .expect("Cannot read current directory");
                let package_manifest_dir = package_manifest_path
                    .parent()
                    .expect("Cannot find parent directory of package manifest");
                package_manifest_dir == current_dir
            }
        })
        .unwrap_or_else(|| {
            rap_error_and_exit("Workspace is not supported.");
        });

    metadata.packages.remove(package_index)
}

fn make_package_with_sorted_target() -> (cargo_metadata::Package, Vec<cargo_metadata::Target>) {
    // Ensure `lib` is compiled before `bin`
    let package = make_package();
    let mut targets: Vec<_> = package.targets.clone().into_iter().collect();
    targets.sort_by_key(|target| TargetKind::from(target) as u8);
    (package, targets)
}

fn clean_package(package_name: &str) {
    let mut cmd = Command::new("cargo");
    cmd.arg("clean")
        .arg("-p")
        .arg(package_name)
        .arg("--target")
        .arg(version_info().host);

    if !cmd
        .spawn()
        .expect("Cannot run cargo clean")
        .wait()
        .expect("Failed to wait for cargo")
        .success() {
        rap_error_and_exit("Cargo clean failed");
    }
}

fn is_identified_target(
    package: &cargo_metadata::Package,
    target: &cargo_metadata::Target,
    cmd: &mut Command
) -> bool {
    match TargetKind::from(target) {
        TargetKind::Library => {
            cmd.arg("--lib");
            cmd.env("RAP_LIB", "");
            clean_package(&package.name);
            true
        },
        TargetKind::Bin => {
            cmd.arg("--bin")
                .arg(&target.name);
            cmd.env("RAP_BIN", &target.name);
            clean_package(&package.name);
            true
        },
        TargetKind::Unspecified => {
            false
        }
    }
}

fn run_cmd(mut cmd: Command, phase: RapPhase) {
    if env::var_os("RAP_VERBOSE").is_some() && phase != RapPhase::Rustc {
        rap_info!("Command is: {:?}", cmd);
    }

    match cmd.status() {
        Ok(status) => {
            if !status.success() {
                process::exit(status.code().unwrap());
            }
        },
        Err(err) => panic!("error in running {:?} {}", cmd, err),
    }
}


fn rap_add_env(cmd: &mut Command) {
    if has_rap_arg_flag("-MIR=V") {
        cmd.env("MIR_DISPLAY", "VERBOSE");
    }
    if has_rap_arg_flag("-MIR=VV") {
        cmd.env("MIR_DISPLAY", "VERY VERBOSE");
    }

    if has_rap_arg_flag("-RCANARY") {
        cmd.env("RCANARY", "ENABLED");
        if has_rap_arg_flag("-ADT=V") {
            cmd.env("ADT_DISPLAY", "");
        }
        if has_rap_arg_flag("-Z3-GOAL=V") {
            cmd.env("Z3_GOAL", "");
        }
        if has_rap_arg_flag("-ICX-SLICE=V") {
            cmd.env("ICX_SLICE", "");
        }
    }

    if has_rap_arg_flag("-SAFEDROP") {
        cmd.env("SAFEDROP", "ENABLED");
    }
}

fn phase_preprocess() {

    let mut args = env::args();
    if args.any(|a| a=="--help") {
        show_help();
    }
    if args.any(|a| a=="--version") {
        show_version();
    }

    rap_info!("Welcome to run RAP - Rust Analysis Platform");
    rap_info!("Ready for RAP Phase I: Preprocess");

    // Make sure that the `rap` and `rustc` binary are from the same sysroot.
    test_sysroot_consistency();

    let mut cmd = Command::new("cargo");
    cmd.arg("clean");
    run_cmd(cmd, RapPhase::PreProcess);
    rap_info!("Running cargo clean for local package");

    rap_remove_dir(RAP_ROOT, "Failed to init RAP root dir");

    rap_info!("Phase-Preprocess has been done");
}

fn llvm_ir_emitter() {
    rap_info!("Ready for RAP Phase II-SubPhase: LLVM-IR-Emitter");
    let (package, targets) = make_package_with_sorted_target();
    for target in targets {

        let mut cmd = Command::new("cargo");
        cmd.arg("rustc")
            .arg("--target-dir")
            .arg(RAP_LLVM_CACHE);

        if !is_identified_target(&package, &target, &mut cmd) {
            continue;
        }

        cmd.arg("--")
            .arg("--emit=llvm-ir")
            .args(RAP_DEFAULT_ARGS);

        if has_arg_flag("-v") || has_arg_flag("-vv") {
            rap_info!("Command is: {:?}", cmd);
        }

        if let Err(e) = cmd.status() {
            rap_error_and_exit(format!("Cannot emit llvm ir: {}", e));
        }

        rap_create_dir(RAP_LLVM_IR, "Failed to creat dir for llvm ir in /tmp");

        for entry in WalkDir::new(RAP_LLVM_CACHE) {
            let entry_path = entry.unwrap().into_path();
            let mut dest_path = PathBuf::from(RAP_LLVM_IR);
            if entry_path
                .iter()
                .last()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".ll")
                &&
                entry_path
                    .iter()
                    .find(|s| s.to_str().unwrap().contains("deps"))
                    .is_some()
            {
                dest_path
                    .push(
                        format!(
                            "{}_{}",
                            TargetKind::from(&target),
                            entry_path
                                .iter()
                                .last()
                                .unwrap()
                                .to_str()
                                .unwrap(),
                        )
                    );

                rap_copy_file(&entry_path, &dest_path, "Failed to copy LLVM IR file");

                rap_info!("Successful to emit LLVM-IR file and transform to: {:?}", dest_path);
            }
        }

        rap_remove_dir(RAP_LLVM_CACHE, "Failed to remove RAP_LLVM_Cache");

    }
    rap_info!("Ready for RAP Phase II-SubPhase: LLVM-IR-Emitter");
}

fn phase_llvm_ir() {
    rap_info!("Ready for RAP Phase II: LLVM-IR");

    llvm_ir_emitter();

    if rap_can_read_dir(RAP_LLVM_IR, "Cannot read LLVM IR files") {
        for entry in WalkDir::new(RAP_LLVM_IR) {
            let path = entry.unwrap().into_path();
            if !path
                .iter()
                .last()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".ll") {
                continue;
            }
            let mut cmd = Command::new("rap_phase_llvm");
            rap_info!("{:?}", path);
            cmd.arg(path);
            if let Err(e) = cmd.status() {
                rap_error_and_exit(format!("RAP-PHASE-LLVM Loaded Failed {}", e));
            }
        }
    } else {
        rap_error_and_exit("Failed to find RAP_LLVM_IR");
    }

    rap_info!("Phase-LLVM-IR has been done");
}


fn phase_cargo_rap() {

    rap_info!("Ready for RAP Phase III: Cargo-RAP");

    let (package, targets) = make_package_with_sorted_target();
    for target in targets {
        let mut args = env::args().skip(2);
        // Now we run `cargo check $FLAGS $ARGS`, giving the user the
        // change to add additional arguments. `FLAGS` is set to identify
        // this target. The user gets to control what gets actually passed to rap.
        let mut cmd = Command::new("cargo");
        cmd.arg("check");

        if !is_identified_target(&package, &target, &mut cmd) {
            continue;
        }

        let verbose = has_arg_flag("-v") || has_arg_flag("-vv");
        if !cfg!(debug_assertions) && !verbose {
            cmd.arg("-q");
        }

        // Forward user-defined `cargo` args until first `--`.
        while let Some(arg) = args.next() {
            if arg == "--" {
                break;
            }
            cmd.arg(arg);
        }

        // Make sure we know the build target, and cargo does, too.
        // This is needed to make the `CARGO_TARGET_*_RUNNER` env var do something,
        // and it later helps us detect which crates are proc-macro/build-script
        // (host crates) and which crates are needed for the program itself.
        let host = version_info().host;
        if  get_arg_flag_value("--target").is_none() {
            // No target given. Pick default and tell cargo about it.
            cmd.arg("--target");
            cmd.arg(&host);
        }

        // Serialize the remaining args into a special environment variable.
        // This will be read by `phase_rustc_rap` when we go to invoke
        // our actual target crate (the binary or the test we are running).
        // Since we're using "cargo check", we have no other way of passing
        // these arguments.
        let args_vec: Vec<String> = args.collect();
        cmd.env(
            "RAP_ARGS",
            serde_json::to_string(&args_vec).expect("failed to serialize args"),
        );


        // Set `RUSTC_WRAPPER` to ourselves.  Cargo will prepend that binary to its usual invocation,
        // i.e., the first argument is `rustc` -- which is what we use in `main` to distinguish
        // the two codepaths. (That extra argument is why we prefer this over setting `RUSTC`.)
        if env::var_os("RUSTC_WRAPPER").is_some() {
            println!(
                "WARNING: Ignoring `RUSTC_WRAPPER` environment variable, RAP does not support wrapping."
            );
        }

        // Invoke actual cargo for the job, but with different flags.
        // We re-use `cargo test` and `cargo run`, which makes target and binary handling very easy but
        // requires some extra work to make the build check-only (see all the `--emit` hacks below).
        // <https://github.com/rust-lang/miri/pull/1540#issuecomment-693553191> describes an alternative
        // approach that uses `cargo check`, making that part easier but target and binary handling
        // harder.
        let cargo_rap_path = env::current_exe().expect("current executable path invalid");
        cmd.env("RUSTC_WRAPPER", &cargo_rap_path);

        if verbose {
            if has_arg_flag("-v") {
                cmd.env("RAP_VERBOSE", "VERBOSE"); // this makes `inside_cargo_rustc` verbose.
            }
            if has_arg_flag("-vv") {
                cmd.env("RAP_VERBOSE", "VERY VERBOSE"); // this makes `inside_cargo_rustc` verbose.
            }
            rap_info!("Command is: {:?}", cmd);
        }

        rap_add_env(&mut cmd);

        rap_info!("Running RAP for target {}:{}", TargetKind::from(&target), &target.name);

        let mut child = cmd
            .spawn()
            .expect("could not run cargo check");
        // 1 hour timeout
        match child
            .wait_timeout(Duration::from_secs(60 * 60))
            .expect("failed to wait for subprocess")
        {
            Some(status) => {
                if !status.success() {
                    rap_error_and_exit("Finished with non-zero exit code");
                }
            }
            None => {
                child.kill().expect("failed to kill subprocess");
                child.wait().expect("failed to wait for subprocess");
                rap_error_and_exit("Killed due to timeout");
            }
        };

    }

    rap_info!("Phase-Cargo-RAP has been done");
}

fn phase_rustc_rap() {
    // Determines if we are being invoked (as rustc) to build a crate for
    // the "target" architecture, in contrast to the "host" architecture.
    // Host crates are for build scripts and proc macros and still need to
    // be built like normal; target crates need to be built for or interpreted
    // by RAP.
    //
    // Currently, we detect this by checking for "--target=", which is
    // never set for host crates. This matches what rustc bootstrap does,
    // which hopefully makes it "reliable enough". This relies on us always
    // invoking cargo itself with `--target`, which `phase_cargo_rap` ensures.
    fn is_target_crate() -> bool {
        get_arg_flag_value("--target").is_some()
    }

    // Determines if we are being invoked to build crate for local crate.
    // Cargo passes the file name as a relative address when building the local crate,
    fn is_current_compile_crate() -> bool {

        fn find_arg_with_rs_suffix() -> Option<String> {
            let mut args = env::args().take_while(|s| s != "--");
            args.find(|s| s.ends_with(".rs"))
        }

        let arg_path = match find_arg_with_rs_suffix() {
            Some(path) => path,
            None => return false,
        };
        let entry_path:&Path = arg_path.as_ref();
        entry_path.is_relative()
    }

    // Determines if the crate being compiled is in the RAP_ADDITIONAL
    // environment variable.
    fn is_additional_compile_crate() -> bool {
        if let (Ok(cargo_pkg_name), Ok(rap_additional)) =
        (env::var("CARGO_PKG_NAME"), env::var("RAP_ADDITIONAL"))
        {
            if rap_additional
                .split(',')
                .any(|s| s.to_lowercase() == cargo_pkg_name.to_lowercase()) {
                return true
            }
        }
        false
    }

    fn is_crate_type_lib() -> bool {
        fn any_arg_flag<F>(name: &str, mut check: F) -> bool
            where
                F: FnMut(&str) -> bool,
        {
            // Stop searching at `--`.
            let mut args = std::env::args().take_while(|val| val != "--");
            loop {
                let arg = match args.next() {
                    Some(arg) => arg,
                    None => return false,
                };
                if !arg.starts_with(name) {
                    continue;
                }

                // Strip leading `name`.
                let suffix = &arg[name.len()..];
                let value = if suffix.is_empty() {
                    // This argument is exactly `name`; the next one is the value.
                    match args.next() {
                        Some(arg) => arg,
                        None => return false,
                    }
                } else if suffix.starts_with('=') {
                    // This argument is `name=value`; get the value.
                    // Strip leading `=`.
                    suffix[1..].to_owned()
                } else {
                    return false;
                };

                if check(&value) {
                    return true;
                }
            }
        }

        any_arg_flag("--crate--type", TargetKind::is_lib_str)
    }

    let is_direct = is_current_compile_crate() && is_target_crate();
    let is_additional = is_additional_compile_crate();

    if is_direct || is_additional {
        let mut cmd = Command::new(find_rap());
        cmd.args(env::args().skip(2));

        // This is the local crate that we want to analyze with RAP.
        // (Testing `target_crate` is needed to exclude build scripts.)
        // We deserialize the arguments that are meant for RAP from the special
        // environment variable "RAP_ARGS", and feed them to the 'RAP' binary.
        //
        // `env::var` is okay here, well-formed JSON is always UTF-8.
        let magic = env::var("RAP_ARGS").expect("missing RAP_ARGS");
        let rap_args: Vec<String> =
            serde_json::from_str(&magic).expect("failed to deserialize RAP_ARGS");
        cmd.args(rap_args);
        run_cmd(cmd, RapPhase::Rustc);
    }
    if !is_direct || is_crate_type_lib() {
        let mut cmd = Command::new("rustc");
        cmd.args(env::args().skip(2));
        run_cmd(cmd, RapPhase::Rustc);
    };

}

#[repr(u8)]
enum TargetKind {
    Library = 0,
    Bin,
    Unspecified,
}

impl Display for TargetKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TargetKind::Library => "lib",
                TargetKind::Bin => "bin",
                TargetKind::Unspecified => "unspecified",
            }
        )
    }
}

impl From<&cargo_metadata::Target> for TargetKind {
    fn from(target: &cargo_metadata::Target) -> Self {
        if target.kind.iter().any(|s| s == "lib" || s == "rlib" || s == "staticlib") {
            TargetKind::Library
        } else if target.kind.iter().any(|s| s == "bin") {
            TargetKind::Bin
        } else {
            TargetKind::Unspecified
        }
    }
}

impl TargetKind {
    fn is_lib_str(s: &str) -> bool {
        s == "lib" || s == "rlib" || s == "staticlib"
    }
}

fn main() {
    // Init the log_system for RAP
    Verbosity::init_rap_log_system_with_verbosity(Verbosity::Info).expect("Failed to set up RAP log system");

    let arg_string = env::args().nth(1).unwrap_or_else(
        || rap_error_and_exit("rap must be called with either `rap` or `rustc` as first argument.")
    );

    if arg_string == String::from("rap") {
        // `cargo rap`: call `cargo rustc` for each applicable target,
        // but with the `RUSTC` env var set to the `cargo-rap` binary so that we come back in the other branch,
        // and dispatch the invocations to `rustc` and `rap`, respectively.
        phase_preprocess();
        // phase_llvm_ir();
        phase_cargo_rap();
    } else if arg_string.ends_with("rustc") {
        // `cargo rap`: `RUSTC_WRAPPER` env var:
        // dependencies get dispatched to `rustc`, the final test/binary to `rap`.

        // this branch is considering the current rust compiler is not directly using bin rustc
        // it will lead to error: 'failed to run `rustc` to learn about target-specific information'
        // cargo will invoke /Users/xx/.cargo/bin/cargo-rap /Users/xx/.rustup/toolchains/stage2/bin/rustc
        // instead of /Users/xx/.cargo/bin/cargo-rap rustc (it is not a dir but is valid in the older version)
        phase_rustc_rap();
    } else {
        rap_error_and_exit("rap must be called with either `rap` or `rustc` as first argument.");
    }

}