use lazy_static::lazy_static;
use log::{Level, LevelFilter, MetadataBuilder, Record};
use env_logger::{Builder, Logger, WriteStyle};
use fern::colors::{Color, ColoredLevelConfig};
use chrono::{Local, Timelike};
use std::{fmt, io::Write};

lazy_static! {
    pub static ref RAP_LOGGER: Logger = {
        let color_line = ColoredLevelConfig::new()
            .info(Color::White)
            .error(Color::Red)
            .warn(Color::Yellow)
            .debug(Color::White)
            .trace(Color::BrightBlack);

        let color_level = color_line.info(Color::Green);
        let builder = Builder::new().format(move |buf, record| {
                let time_now = Local::now();
                writeln!(buf,
                    "{}{}:{}|RAP-BACK|{}{}|: {}\x1B[0m",
                    format_args!("\x1B[{}m",color_line.get_color(&record.level()).to_fg_str()),
                    time_now.hour(),
                    time_now.minute(),
                    color_level.color(record.level()),
                    format_args!("\x1B[{}m",color_line.get_color(&record.level()).to_fg_str()),
                    record.args()
                )
            }).filter(None, LevelFilter::Info)
            .write_style(WriteStyle::Always)
            .build();
        builder
    };
}

#[derive(Debug, Copy, Clone, Hash)]
pub enum RapLogLevel {
    Info,
    Warn,
    Error,
}

pub fn record_msg(args: fmt::Arguments<'_>, level: RapLogLevel) -> Record<'_> {
    let meta = MetadataBuilder::new().target("RAP").level(
            match level {
                RapLogLevel::Info => Level::Info,
                RapLogLevel::Warn => Level::Warn,
                RapLogLevel::Error => Level::Error,
            }
        ).build();
    let record = Record::builder().metadata(meta).args(args.clone()).build();
    record
}

#[macro_export]
macro_rules! rap_info {
    ($($arg:tt)+) => (
        RAP_LOGGER.log(&record_msg(format_args!($($arg)+), RapLogLevel::Info))
    );
}

#[macro_export]
macro_rules! rap_error {
    ($($arg:tt)+) => (
        RAP_LOGGER.log(&record_msg(format_args!($($arg)+), RapLogLevel::Error))
    );
}

#[macro_export]
macro_rules! rap_warn {
    ($($arg:tt)+) => (
        RAP_LOGGER.log(&record_msg(format_args!($($arg)+), RapLogLevel::Warn))
    );
}
