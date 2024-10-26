use std::{
    env,
    path::{Path, PathBuf},
    sync::LazyLock,
};

struct Arguments {
    /// a collection of `std::env::args()`
    args: Vec<String>,
    /// options as first half before -- in args
    args_group1: Vec<String>,
    /// options as second half after -- in args
    args_group2: Vec<String>,
    current_exe_path: PathBuf,
}

impl Arguments {
    // Get value from `name=val` or `name val`.
    fn get_arg_flag_value(&self, name: &str) -> Option<&str> {
        let mut args = self.args_group1.iter();

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
        let args: Vec<_> = env::args().collect();
        let path = env::current_exe().expect("Current executable path invalid.");
        rap_debug!("Current exe: {path:?}\tReceived args: {args:?}");
        let [args_group1, args_group2] = split_args_by_double_dash(&args);
        Arguments {
            args,
            args_group1,
            args_group2,
            current_exe_path: path,
        }
    }

    // In rustc phase:
    // Determines if we are being invoked to build crate for local crate.
    // Cargo passes the file name as a relative address when building the local crate,
    fn is_current_compile_crate(&self) -> bool {
        let mut args = self.args_group1.iter();
        let entry_path = match args.find(|s| s.ends_with(".rs")) {
            Some(path) => Path::new(path),
            None => return false,
        };
        entry_path.is_relative()
    }
}

fn split_args_by_double_dash(args: &[String]) -> [Vec<String>; 2] {
    let mut args = args.iter().skip(2).map(|arg| arg.to_owned());
    let rap_args = args.by_ref().take_while(|arg| *arg != "--").collect();
    let cargo_args = args.collect();
    [rap_args, cargo_args]
}

static ARGS: LazyLock<Arguments> = LazyLock::new(Arguments::new);

pub fn get_arg_flag_value(name: &str) -> Option<&'static str> {
    ARGS.get_arg_flag_value(name)
}

/// `cargo rap [rap options] -- [cargo check options]`
///
/// Options before the first `--` are arguments forwarding to rap.
/// Stuff all after the first `--` are arguments forwarding to cargo check.
pub fn rap_and_cargo_args() -> [&'static [String]; 2] {
    [&ARGS.args_group1, &ARGS.args_group2]
}

/// If a crate being compiled is local in rustc phase.
pub fn is_current_compile_crate() -> bool {
    ARGS.is_current_compile_crate()
}

pub fn is_crate_type_lib() -> bool {
    get_arg_flag_value("--crate--type")
        .map(crate::TargetKind::is_lib_str)
        .unwrap_or(false)
}

pub fn get_arg(pos: usize) -> Option<&'static str> {
    ARGS.args.get(pos).map(|x| x.as_str())
}

pub fn skip2() -> &'static [String] {
    ARGS.args.get(2..).unwrap_or(&[])
}

pub fn current_exe_path() -> &'static Path {
    &ARGS.current_exe_path
}
