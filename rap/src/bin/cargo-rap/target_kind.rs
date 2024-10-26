use cargo_metadata::{Metadata, MetadataCommand, Target};
use rap::{rap_debug, rap_error, rap_info};
use std::{
    fmt::{Display, Formatter},
    process::Command,
};

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

pub fn is_identified_target(target: &Target, cmd: &mut Command) -> bool {
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
pub fn find_targets(metadata: &mut Metadata) -> Vec<Target> {
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
