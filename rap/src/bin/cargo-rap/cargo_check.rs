use crate::args;
use cargo_metadata::{
    camino::{Utf8Path, Utf8PathBuf},
    Metadata,
};
use rap::utils::log::rap_error_and_exit;
use std::{collections::BTreeMap, process::Command, time::Duration};
use wait_timeout::ChildExt;

pub fn run(dir: &Utf8Path) {
    let [rap_args, cargo_args] = args::rap_and_cargo_args();
    rap_debug!("rap_args={rap_args:?}\tcargo_args={cargo_args:?}");

    /*Here we prepare the cargo command as cargo check, which is similar to build, but much faster*/
    let mut cmd = Command::new("cargo");
    cmd.current_dir(dir);
    cmd.arg("check");

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
    let cargo_rap_path = args::current_exe_path();
    cmd.env("RUSTC_WRAPPER", cargo_rap_path);

    rap_debug!("Command is: {:?}.", cmd);

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

pub fn get_cargo_tomls_deep_recursively(dir: &Utf8Path) -> Vec<Utf8PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|entry| {
            if let Ok(e) = entry {
                if e.file_type().is_file() && e.file_name().to_str()? == "Cargo.toml" {
                    let path = Utf8PathBuf::from_path_buf(e.into_path());
                    return path.ok()?.canonicalize_utf8().ok();
                }
            }
            None
        })
        .collect()
}

type Workspaces = BTreeMap<Utf8PathBuf, Metadata>;

fn workspaces(cargo_tomls: &[Utf8PathBuf]) -> Workspaces {
    let mut map = BTreeMap::new();
    for cargo_toml in cargo_tomls {
        let metadata = workspace(cargo_toml);
        let root = &metadata.workspace_root;
        // 每个 member package 解析的 workspace_root 和 members 是一样的
        if !map.contains_key(root) {
            map.insert(root.clone(), metadata);
        }
    }

    map
}

fn workspace(cargo_toml: &Utf8Path) -> Metadata {
    let exec = cargo_metadata::MetadataCommand::new()
        .manifest_path(cargo_toml)
        .exec();
    let metadata = match exec {
        Ok(metadata) => metadata,
        Err(err) => {
            let err = format!("Failed to get result of cargo metadata: \n{err}");
            rap_error_and_exit(err)
        }
    };
    metadata
}

fn get_member_folders(meta: &Metadata) -> Vec<&Utf8Path> {
    meta.workspace_packages()
        .iter()
        .map(|pkg| pkg.manifest_path.parent().unwrap())
        .collect()
}

/// Just like running a cargo check in a folder.
pub fn default_run() {
    run(".".into());
}

/// Run cargo check in each member folder under current workspace.
pub fn shallow_run() {
    let metadata = workspace(".".into());
    for pkg_folder in get_member_folders(&metadata) {
        run(pkg_folder);
    }
}
