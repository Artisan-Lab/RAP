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
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::White)
        .debug(Color::Blue)
        .trace(Color::Cyan);

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
macro_rules! rap_trace {
    ($($arg:tt)+) => (
        ::log::trace!(target: "RAP", $($arg)+)
    );
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
macro_rules! rap_warn {
    ($($arg:tt)+) => (
        ::log::warn!(target: "RAP", $($arg)+)
    );
}

#[macro_export]
macro_rules! rap_error {
    ($($arg:tt)+) => (
        ::log::error!(target: "RAP", $($arg)+)
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
pub fn span_to_first_line(span: Span) -> Span {
    // extend the span to an entrie line or extract the first line if it has multiple lines
    get_source_map()
        .unwrap()
        .span_extend_to_line(span.shrink_to_lo())
}

#[inline]
pub fn span_to_trimmed_span(span: Span) -> Span {
    // trim out the first few whitespace
    span.trim_start(
        get_source_map()
            .unwrap()
            .span_take_while(span, |c| c.is_whitespace()),
    )
    .unwrap()
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
// this function computes the relative pos range of two spans which could be generated from two dirrerent files or not intersect with each other
pub unsafe fn relative_pos_range(span: Span, sub_span: Span) -> Range<usize> {
    if sub_span.lo() < span.lo() || sub_span.hi() > span.hi() {
        return 0..0;
    }
    let offset = span.lo();
    let lo = (sub_span.lo() - offset).to_usize();
    let hi = (sub_span.hi() - offset).to_usize();
    lo..hi
}

pub fn are_spans_in_same_file(span1: Span, span2: Span) -> bool {
    let file1 = get_source_map().unwrap().lookup_source_file(span1.lo());
    let file2 = get_source_map().unwrap().lookup_source_file(span2.lo());
    file1.name == file2.name
}
