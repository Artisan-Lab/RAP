use std::sync::LazyLock;

struct Arguments {
    /// a collection of `std::env::args()`
    args: Vec<String>,
    /// options as first half before -- in args
    rap_args: Vec<String>,
    /// options as second half after -- in args
    cargo_args: Vec<String>,
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
        let args: Vec<_> = std::env::args().collect();
        let [rap_args, cargo_args] = new_rap_and_cargo_args(&args);
        Arguments {
            args,
            rap_args,
            cargo_args,
        }
    }
}

/// `cargo rap [rap options] -- [cargo check options]`
///
/// Options before the first `--` are arguments forwarding to rap.
/// Stuff all after the first `--` are arguments forwarding to cargo check.
fn new_rap_and_cargo_args(args: &[String]) -> [Vec<String>; 2] {
    let mut args = args.iter().skip(2).map(|arg| arg.to_owned());
    let rap_args = args.by_ref().take_while(|arg| *arg != "--").collect();
    let cargo_args = args.collect();
    [rap_args, cargo_args]
}

static ARGS: LazyLock<Arguments> = LazyLock::new(Arguments::new);

pub fn get_arg_flag_value(name: &str) -> Option<&'static str> {
    ARGS.get_arg_flag_value(name)
}

///  Get rap & cargo check options from
///  `cargo rap [rap options] -- [cargo check options]`.
pub fn rap_and_cargo_args() -> [&'static [String]; 2] {
    [&ARGS.rap_args, &ARGS.cargo_args]
}
