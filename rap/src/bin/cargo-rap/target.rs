use cargo_metadata::{Metadata, MetadataCommand, Target};
use std::{
    env,
    fmt::{Display, Formatter},
    process::Command,
    time::Duration,
};

use rap::utils::log::{init_log, rap_error_and_exit};
use wait_timeout::ChildExt;

#[repr(u8)]
pub enum TargetKind {
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

impl From<&Target> for TargetKind {
    fn from(target: &Target) -> Self {
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
    pub fn is_lib_str(s: &str) -> bool {
        s == "lib" || s == "rlib" || s == "staticlib"
    }
}

fn is_identified_target(target: &Target, cmd: &mut Command) -> bool {
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

/*
    The function finds a package under the current directory.
*/
fn find_targets(metadata: &mut Metadata) -> Vec<Target> {
    rap_info!("Search local targets for analysis.");
    let current_dir = std::env::current_dir();
    let current_dir = current_dir.as_ref().expect("Cannot read current dir.");
    let mut pkg_iter = metadata.packages.iter().filter(|package| {
        let package_dir = package
            .manifest_path
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

pub fn run_cargo_check() {
    let cmd = MetadataCommand::new();
    rap_debug!("Please run `cargo metadata` if this step takes too long");
    let mut metadata = match cmd.exec() {
        // execute command: `cargo metadata'
        Ok(metadata) => metadata,
        Err(e) => rap_error_and_exit(format!("Cannot obtain cargo metadata: {}.", e)),
    };

    let [rap_args, cargo_args] = crate::args::rap_and_cargo_args();
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
        cmd.args(cargo_args);

        // Serialize the remaining args into a special environment variable.
        // This will be read by `phase_rustc_rap` when we go to invoke
        // our actual target crate (the binary or the test we are running).

        cmd.env(
            "RAP_ARGS",
            serde_json::to_string(rap_args).expect("Failed to serialize args."),
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
