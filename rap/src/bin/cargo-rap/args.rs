use std::sync::LazyLock;

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
            args: std::env::args().collect(),
        }
    }

    /// `cargo rap [rap options] -- [cargo check options]`
    ///
    /// Options before the first `--` are arguments forwarding to rap.
    /// Stuff all after the first `--` are arguments forwarding to cargo check.
    fn rap_and_cargo_args(&self) -> [Vec<&str>; 2] {
        let mut args = self.args.iter().map(|arg| arg.as_str()).skip(2);
        let rap_args = args.by_ref().take_while(|arg| *arg != "--").collect();
        let cargo_args = args.collect();
        [rap_args, cargo_args]
    }
}

static ARGS: LazyLock<Arguments> = LazyLock::new(Arguments::new);

pub fn get_arg_flag_value(name: &str) -> Option<&'static str> {
    ARGS.get_arg_flag_value(name)
}

pub fn rap_and_cargo_args() -> [Vec<&'static str>; 2] {
    ARGS.rap_and_cargo_args()
}
