use chrono::Local;
use fern::colors::{Color, ColoredLevelConfig};
use fern::{self, Dispatch};
use log::LevelFilter;
use rustc_span::source_map::get_source_map;
use rustc_span::{BytePos, Pos, Span};

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

pub fn underline_span_in_the_line(span: Span) -> String {
    fn compose_underline(line_span: Span, span: Span) -> String {
        let line_len = (line_span.hi() - line_span.lo()).to_u32();
        let line_start_pos = line_span.lo();
        let lo = (span.lo() - line_start_pos).to_u32();
        let hi = (span.hi() - line_start_pos).to_u32();
        (0..line_len)
            .map(|i| if i >= lo && i < hi { '^' } else { ' ' })
            .collect()
    }

    let source_map = get_source_map().unwrap();
    let line_span = source_map.span_extend_to_line(span);
    let line = source_map.span_to_snippet(line_span).unwrap();
    let underline = compose_underline(line_span, span);
    format!(
        "{}\n{}\n{}",
        source_map.span_to_diagnostic_string(span),
        line,
        underline
    )
}
