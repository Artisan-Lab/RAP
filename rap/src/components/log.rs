use std::env;

use chrono::{Datelike, Local, Timelike};
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
    pub fn init_rap_log_system_with_verbosity(verbose: Verbosity) -> Result<(), fern::InitError> {
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
        }.level_for(
            "rap",
            if cfg!(debug_assertion) {LevelFilter::Debug} else {LevelFilter::Info}
        );

        if let Some(log_file_path) = env::var_os("RAP_LOG_FILE_PATH") {
            let file_dispatch = Dispatch::new()
                .filter(|metadata| metadata.target() == "=RAP=")
                .format(|callback, args, record| {
                    callback.finish(format_args!(
                        "{} |RAP OUTPUT-{:5}| {}",
                        Local::now().date_naive(),
                        record.level(),
                        args,
                    ))
                })
                .chain(fern::log_file(log_file_path)?);
            dispatch = dispatch.chain(file_dispatch);
        }

        let stdout_dispatch = Dispatch::new()
            .format(move |callback, args, record| {
                let time_now = Local::now();
                callback.finish(format_args!(
                    "{}{}-{}:{}:{}:{} |FRONT| |{:5}{}| [{}] {}\x1B[0m",
                    format_args!(
                        "\x1B[{}m",
                        color_line.get_color(&record.level()).to_fg_str()
                    ),
                    time_now.month(),
                    time_now.day(),
                    time_now.hour(),
                    time_now.minute(),
                    time_now.second(),
                    color_level.color(record.level()),
                    format_args!(
                        "\x1B[{}m",
                        color_line.get_color(&record.level()).to_fg_str()
                    ),
                    record.target(),
                    args
                ))
            })
            .level(log::LevelFilter::Info)
            .chain(std::io::stdout());

        dispatch.chain(stdout_dispatch).apply()?;
        Ok(())
    }
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
