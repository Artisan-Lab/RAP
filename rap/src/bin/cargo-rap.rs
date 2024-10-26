/*
    This is a cargo program to start RAP.
    The file references the cargo file for Miri: https://github.com/rust-lang/miri/blob/master/cargo-miri/src/main.rs
*/
use cargo_metadata::{Metadata, MetadataCommand};
use rap::utils::log::{init_log, rap_error_and_exit};
use rap::{rap_debug, rap_error, rap_info};
use std::env;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::sync::LazyLock;
use std::time::Duration;
use wait_timeout::ChildExt;

const RAP_HELP: &str = r#"
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

e.g. detect use-after-free and memory leak for a riscv target:
    cargo rap -F -M -- --target riscv64gc-unknown-none-elf
"#;

const RAP_VERSION: &str = r#"
rap version 0.1
released at 2024-07-23
developped by artisan-lab @ Fudan university 
"#;

struct Arguments {
    /// a collection of `std::env::args()`
    args: Vec<String>,
}

impl Arguments {
    // Get value from `name=val` or `name val`.
    fn get_arg_flag_value(&self, name: &str) -> Option<&str> {
        let mut args = self.args.iter().take_while(|val| *val != "--");

        while let Some(arg) = args.next() {
            if !arg.starts_with(name) {
                continue;
            }
            // Strip leading `name`.
            let suffix = &arg[name.len()..];
            if suffix.is_empty() {
                // This argument is exactly `name`; the next one is the value.
                return args.next().map(|x| x.as_str());
            } else if suffix.starts_with('=') {
                // This argument is `name=value`; get the value.
                // Strip leading `=`.
                return Some(&suffix[1..]);
            }
        }

        None
    }

    fn new() -> Self {
        Arguments {
            args: env::args().collect(),
        }
    }

    /// `cargo rap [rap options] -- [cargo check options]`
    ///
    /// Options before the first `--` are arguments forwarding to rap.
    /// Stuff all after the first `--` are arguments forwarding to cargo check.
    fn rap_and_cargo_args(&self) -> [Vec<&str>; 2] {
        dbg!(&self.args);
        let mut args = self.args.iter().map(|arg| arg.as_str()).skip(2);
        let rap_args = args.by_ref().take_while(|arg| *arg != "--").collect();
        let cargo_args = args.collect();
        [rap_args, cargo_args]
    }
}

static ARGS: LazyLock<Arguments> = LazyLock::new(Arguments::new);

fn get_arg_flag_value(name: &str) -> Option<&'static str> {
    ARGS.get_arg_flag_value(name)
}

fn rap_and_cargo_args() -> [Vec<&'static str>; 2] {
    ARGS.rap_and_cargo_args()
}

fn find_rap() -> PathBuf {
    let mut path = env::current_exe().expect("Current executable path invalid.");
    path.set_file_name("rap");
    path
}

/*
    The function finds a package under the current directory.
*/
fn find_targets(metadata: &mut Metadata) -> Vec<cargo_metadata::Target> {
    rap_info!("Search local targets for analysis.");
    let current_dir = env::current_dir();
    let current_dir = current_dir.as_ref().expect("Cannot read current dir.");
    let mut pkg_iter = metadata.packages.iter().filter(|package| {
        let package_dir = Path::new(&package.manifest_path)
            .parent()
            .expect("Failed to find parent directory.");
        rap_debug!("Package_dir: {:?}.", package_dir);
        //FIXME: do we need to handle sub directories?
        package_dir == current_dir || package_dir.starts_with(&current_dir.to_str().unwrap())
    });
    let mut targets = Vec::new();
    while let Some(pkg) = pkg_iter.next() {
        rap_info!("Find a new pakage: {:?}.", pkg.name);
        let mut pkg_targets: Vec<_> = pkg.targets.clone().into_iter().collect();
        // Ensure `lib` is compiled before `bin`
        pkg_targets.sort_by_key(|target| TargetKind::from(target) as u8);
        targets.extend(pkg_targets);
    }
    targets
}

fn is_identified_target(target: &cargo_metadata::Target, cmd: &mut Command) -> bool {
    match TargetKind::from(target) {
        TargetKind::Library => {
            cmd.arg("--lib");
            true
        }
        TargetKind::Bin => {
            cmd.arg("--bin").arg(&target.name);
            true
        }
        TargetKind::Unspecified => false,
    }
}

fn run_cmd(mut cmd: Command) {
    rap_debug!("Command is: {:?}.", cmd);
    match cmd.status() {
        Ok(status) => {
            if !status.success() {
                process::exit(status.code().unwrap());
            }
        }
        Err(err) => panic!("Error in running {:?} {}.", cmd, err),
    }
}

fn phase_cargo_rap() {
    rap_info!("Start cargo-rap");
    let mut args = env::args().skip(2); // here we skip two tokens: cargo rap
    let Some(arg) = args.next() else {
        rap_error!("Expect command: e.g., `cargo rap -help`.");
        return;
    };
    match arg.as_str() {
        "-V" | "-version" => {
            rap_info!("{}", RAP_VERSION);
            return;
        }
        "-H" | "-help" | "--help" => {
            rap_info!("{}", RAP_HELP);
            return;
        }
        _ => {}
    }

    let cmd = MetadataCommand::new();
    rap_debug!("Please run `cargo metadata` if this step takes too long");
    let mut metadata = match cmd.exec() {
        // execute command: `cargo metadata'
        Ok(metadata) => metadata,
        Err(e) => rap_error_and_exit(format!("Cannot obtain cargo metadata: {}.", e)),
    };

    let [rap_args, cargo_args] = rap_and_cargo_args();
    rap_debug!("rap_args={rap_args:?}\tcargo_args={cargo_args:?}");

    let targets = find_targets(&mut metadata);
    for target in targets {
        /*Here we prepare the cargo command as cargo check, which is similar to build, but much faster*/
        let mut cmd = Command::new("cargo");
        cmd.arg("check");

        /* We only process bin and lib targets, and ignore others */
        if !is_identified_target(&target, &mut cmd) {
            rap_debug!("Ignore the target because it is neither bin or lib.");
            continue;
        }

        /* set the target as a filter for phase_rustc_rap */
        cmd.args(&cargo_args);

        // Serialize the remaining args into a special environment variable.
        // This will be read by `phase_rustc_rap` when we go to invoke
        // our actual target crate (the binary or the test we are running).

        cmd.env(
            "RAP_ARGS",
            serde_json::to_string(&rap_args).expect("Failed to serialize args."),
        );

        // Invoke actual cargo for the job, but with different flags.
        let cargo_rap_path = env::current_exe().expect("Current executable path is invalid.");
        cmd.env("RUSTC_WRAPPER", &cargo_rap_path);

        rap_debug!("Command is: {:?}.", cmd);
        rap_info!(
            "Running rap for target {}:{}",
            TargetKind::from(&target),
            &target.name
        );

        let mut child = cmd.spawn().expect("Could not run cargo check.");
        match child
            .wait_timeout(Duration::from_secs(60 * 60)) // 1 hour timeout
            .expect("Failed to wait for subprocess.")
        {
            Some(status) => {
                if !status.success() {
                    rap_error_and_exit("Finished with non-zero exit code.");
                }
            }
            None => {
                child.kill().expect("Failed to kill subprocess.");
                child.wait().expect("Failed to wait for subprocess.");
                rap_error_and_exit("Process killed due to timeout.");
            }
        };
    }
}

fn phase_rustc_wrapper() {
    rap_debug!("Launch cargo-rap again triggered by cargo check.");

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
        let entry_path: &Path = arg_path.as_ref();
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

    let is_direct = is_current_compile_crate();
    if is_direct {
        let mut cmd = Command::new(find_rap());
        cmd.args(env::args().skip(2));
        let magic = env::var("RAP_ARGS").expect("Missing RAP_ARGS.");
        let rap_args: Vec<String> =
            serde_json::from_str(&magic).expect("Failed to deserialize RAP_ARGS.");
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
    Library,
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
        if target
            .kind
            .iter()
            .any(|s| s == "lib" || s == "rlib" || s == "staticlib")
        {
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

    // Init the log_system
    init_log().expect("Failed to init log.");
    rap_debug!("Enter cargo-rap; Received args: {:?}", env::args());

    let first_arg = env::args().nth(1);
    match first_arg.unwrap() {
        s if s.ends_with("rap") => phase_cargo_rap(),
        s if s.ends_with("rustc") => phase_rustc_wrapper(),
        _ => {
            rap_error_and_exit("rap must be called with either `rap` or `rustc` as first argument.")
        }
    }
}
