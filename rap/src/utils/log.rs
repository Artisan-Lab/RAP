use chrono::{Local, Timelike};
use fern::{self, Dispatch};
use fern::colors::{Color, ColoredLevelConfig};
use log::LevelFilter;

#[derive(Debug, Copy, Clone, Hash)]
pub enum Verbosity {
    Info,
    Debug,
    Trace,
}

impl Verbosity {
    pub fn init_log(verbose: Verbosity) -> Result<(), fern::InitError> {
        let mut dispatch = Dispatch::new();
        let color_line = ColoredLevelConfig::new()
            .info(Color::White)
            .error(Color::Red)
            .warn(Color::Yellow)
            .debug(Color::White)
            .trace(Color::BrightBlack);

        let color_level = color_line.info(Color::Green);
        dispatch = match verbose {
            Verbosity::Info => dispatch.level(LevelFilter::Info),
            Verbosity::Debug => dispatch.level(LevelFilter::Debug),
            Verbosity::Trace => dispatch.level(LevelFilter::Trace),
        };//.level_for("rap", LevelFilter::Debug);

        let stderr_dispatch = Dispatch::new()
            .format(move |callback, args, record| {
                let time_now = Local::now();
                callback.finish(format_args!(
                    "{}{}:{}|RAP-FRONT|{}{}|: {}\x1B[0m",
                    format_args!("\x1B[{}m",color_line.get_color(&record.level()).to_fg_str()),
                    time_now.hour(),
                    time_now.minute(),
                    color_level.color(record.level()),
                    format_args!("\x1B[{}m",color_line.get_color(&record.level()).to_fg_str()),
                    args
                ))
            }).chain(std::io::stderr());

        /* Note that we cannot dispatch to stdout due to some bugs */
        dispatch.chain(stderr_dispatch).apply()?;
        Ok(())
    }
}

#[macro_export]
macro_rules! rap_debug {
    ($($arg:tt)+) => (
        ::log::debug!(target: "RAP", $($arg)+)
    );
}

#[macro_export]
macro_rules! rap_info {
    ($($arg:tt)+) => (
        ::log::info!(target: "RAP", $($arg)+)
    );
}

#[macro_export]
macro_rules! rap_error {
    ($($arg:tt)+) => (
        ::log::error!(target: "RAP", $($arg)+)
    );
}

#[macro_export]
macro_rules! rap_warn {
    ($($arg:tt)+) => (
        ::log::warn!(target: "RAP", $($arg)+)
    );
}

pub fn rap_error_and_exit(msg: impl AsRef<str>) -> ! {
    rap_error!("{}", msg.as_ref());
    std::process::exit(1)
}
