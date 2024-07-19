/* 
    This is a cargo program to start RAP.
    The file references the cargo file for Miri: https://github.com/rust-lang/miri/blob/master/cargo-miri/src/main.rs
*/

use rap::{rap_info, rap_debug, rap_error};
use rap::utils::log::{Verbosity, rap_error_and_exit};
use std::env;
use std::iter::TakeWhile;
use std::process::Command;
use std::path::{PathBuf, Path};
use std::time::Duration;
use std::fmt::{Display, Formatter};
use std::process;
use rustc_version::VersionMeta;
use wait_timeout::ChildExt;

const CARGO_RAP_HELP: &str = r#"Run RAP to test and check Rust crates

Usage:
    cargo rap [options...]

Use-After-Free/Double Free detection.
    -F or -uaf     Command: "cargo rap -uaf"

Memory leakage detection.
    -M or -mleak     Command: "cargo rap -mleak"

Unsafe code tracing
    -UI or -uig      generate unsafe code isolation graph

General command: 
    -H or -help:     show help information
    -V or -version:  show the version of RAP

Debugging options:
    -debug	     show the debug-level logs
    -mir             print the MIR of each function

"#;

fn has_arg_flag(name: &str) -> bool {
    let mut args = env::args().skip(0);
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

fn get_arg_flag_value(name: &str) -> Option<String> {
    ArgFlagValueIter::new(name).next()
}

fn find_rap() -> PathBuf {
    let mut path = env::current_exe().expect("current executable path invalid");
    path.set_file_name("rap");
    path
}

fn version_info() -> VersionMeta {
    let rap = Command::new(find_rap());
    VersionMeta::for_command(rap).expect("failed to determine underlying rustc version of RAP")
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
    let rap_sysroot = get_sysroot(Command::new(find_rap()));

    assert_eq!(
        rustc_sysroot,
        rap_sysroot,
        "Inconsistent toolchain! You may switch the default toolchain via !\n\
         `rustup default rap-rust`"
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

fn make_package_with_sorted_target() -> Vec<cargo_metadata::Target> {
    // Ensure `lib` is compiled before `bin`
    let package = make_package();
    let mut targets: Vec<_> = package.targets.clone().into_iter().collect();
    targets.sort_by_key(|target| TargetKind::from(target) as u8);
    targets
}

fn is_identified_target(
    target: &cargo_metadata::Target,
    cmd: &mut Command
) -> bool {
    match TargetKind::from(target) {
        TargetKind::Library => {
            cmd.arg("--lib");
            true
        },
        TargetKind::Bin => {
            cmd.arg("--bin").arg(&target.name);
            true
        },
        TargetKind::Unspecified => {
            false
        }
    }
}

fn run_cmd(mut cmd: Command) {
    rap_debug!("Command is: {:?}", cmd);

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
    if has_arg_flag("-F") || has_arg_flag("-uaf") {
        cmd.env("UAF", "ENABLED");
    }
    if has_arg_flag("-mir") {
        cmd.env("MIR_SHOW", "ON");
    }
}

fn cleanup(){ 
    let mut cmd = Command::new("cargo");
    cmd.arg("clean");
    run_cmd(cmd);
    rap_info!("Execute cargo clean.");
}

fn phase_cargo_rap() {
    test_sysroot_consistency();
    cleanup(); // clean up the directory before building.
    let mut args = env::args().skip(2); // here we skip two tokens: cargo rap
    let Some(arg) = args.next() else {
        rap_error!("expect command: e.g., `cargo rap -help`");
	return ;
    };
    match arg.as_str() {
        "-V" | "-version" => { rap_info!("The RAP version: {}", "0.1"); return; },
        "-H" | "-help" | "--help" => { rap_info!("{}", CARGO_RAP_HELP); return; },
	_ => {},
    }

    let targets = make_package_with_sorted_target();
    for target in targets {
	/*Here we prepare the cargo command as cargo check, which is similar to build, but much faster*/
        let mut cmd = Command::new("cargo");
        cmd.arg("check"); 

	/* We only process bin and lib targets, and ignore others */
        if !is_identified_target(&target, &mut cmd) {
            continue;
        }

        /* set the target as a filter for phase_rustc_rap */
        let host = version_info().host;
        if  get_arg_flag_value("--target").is_none() {
            cmd.arg("--target");
            cmd.arg(&host);
        }

        // Serialize the remaining args into a special environment variable.
        // This will be read by `phase_rustc_rap` when we go to invoke
        // our actual target crate (the binary or the test we are running).

        let args = env::args().skip(2);
        let args_vec: Vec<String> = args.collect();
        cmd.env(
            "RAP_ARGS",
            serde_json::to_string(&args_vec).expect("failed to serialize args"),
        );

        // Invoke actual cargo for the job, but with different flags.
        let cargo_rap_path = env::current_exe().expect("current executable path invalid");
        cmd.env("RUSTC_WRAPPER", &cargo_rap_path);

        rap_debug!("Command is: {:?}", cmd);
        rap_add_env(&mut cmd);
        rap_info!("Running rap for target {}:{}", TargetKind::from(&target), &target.name);

        let mut child = cmd
            .spawn()
            .expect("could not run cargo check");
        match child.wait_timeout(Duration::from_secs(60 * 60)) // 1 hour timeout
            .expect("failed to wait for subprocess") {
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

fn phase_rustc_wrapper() {
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

    if is_direct {
        let mut cmd = Command::new(find_rap());
        cmd.args(env::args().skip(2));
        let magic = env::var("RAP_ARGS").expect("missing RAP_ARGS");
        let rap_args: Vec<String> =
            serde_json::from_str(&magic).expect("failed to deserialize RAP_ARGS");
        cmd.args(rap_args);
        run_cmd(cmd);
    }
    if !is_direct || is_crate_type_lib() {
        let mut cmd = Command::new("rustc");
        cmd.args(env::args().skip(2));
        run_cmd(cmd);
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
    /* This function will be enteredd twice:
       1. When we run `cargo rap ...`, cargo dispatches the execution to cargo-rap.
	  In this step, we set RUSTC_WRAPPER to cargo-rap, and execute `cargo check ...` command;
       2. Cargo check actually triggers `path/cargo-rap path/rustc` according to RUSTC_WRAPPER.
          Because RUSTC_WRAPPER is defined, Cargo calls the command: `$RUSTC_WRAPPER path/rustc ...`
    */
    // Init the log_system for RAP
    Verbosity::init_log(Verbosity::Info).expect("Failed to init log");
    //Verbosity::init_log(Verbosity::Debug).expect("Failed to init log");

    rap_info!("Enter cargo-rap...");
    rap_debug!("Received args: {:?}", env::args());

    let first_arg = env::args().nth(1);
    match first_arg.unwrap() {
       s if s.ends_with("rap") => phase_cargo_rap(),
       s if s.ends_with("rustc") => phase_rustc_wrapper(),
       _ => rap_error_and_exit("rap must be called with either `rap` or `rustc` as first argument."),
    }
}
