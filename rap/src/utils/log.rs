use chrono::Local;
use fern::colors::{Color, ColoredLevelConfig};
use fern::{self, Dispatch};
use log::LevelFilter;
use rustc_span::source_map::get_source_map;
use rustc_span::{FileNameDisplayPreference, Pos, Span};
use std::ops::Range;

fn log_level() -> LevelFilter {
    if let Ok(s) = std::env::var("RAP_LOG") {
        match s.parse() {
            Ok(level) => return level,
            Err(err) => eprintln!("RAP_LOG is invalid: {err}"),
        }
    }
    LevelFilter::Info
}

/// Detect `RAP_LOG` environment variable first; if it's not set,
/// default to INFO level.
pub fn init_log() -> Result<(), fern::InitError> {
    let dispatch = Dispatch::new().level(log_level());

    let color_line = ColoredLevelConfig::new()
        .info(Color::White)
        .error(Color::Red)
        .warn(Color::Yellow)
        .debug(Color::White)
        .trace(Color::BrightBlack);

    let color_level = color_line.info(Color::Green);
    let stderr_dispatch = Dispatch::new()
        .format(move |callback, args, record| {
            let now = Local::now();
            callback.finish(format_args!(
                "{}{}|RAP|{}{}|: {}\x1B[0m",
                format_args!(
                    "\x1B[{}m",
                    color_line.get_color(&record.level()).to_fg_str()
                ),
                now.format("%H:%M:%S"),
                color_level.color(record.level()),
                format_args!(
                    "\x1B[{}m",
                    color_line.get_color(&record.level()).to_fg_str()
                ),
                args
            ))
        })
        .chain(std::io::stderr());

    /* Note that we cannot dispatch to stdout due to some bugs */
    dispatch.chain(stderr_dispatch).apply()?;
    Ok(())
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

#[inline]
pub fn span_to_source_code(span: Span) -> String {
    get_source_map().unwrap().span_to_snippet(span).unwrap()
}

#[inline]
pub fn span_to_filename(span: Span) -> String {
    get_source_map()
        .unwrap()
        .span_to_filename(span)
        .display(FileNameDisplayPreference::Local)
        .to_string()
}

#[inline]
pub fn span_to_line_number(span: Span) -> usize {
    get_source_map().unwrap().lookup_char_pos(span.lo()).line
}

#[inline]
pub fn relative_pos_range(span: Span, sub_span: Span) -> Range<usize> {
    let start_pos = span.lo();
    let lo = (sub_span.lo() - start_pos).to_usize();
    let hi = (sub_span.hi() - start_pos).to_usize();
    lo..hi
}
