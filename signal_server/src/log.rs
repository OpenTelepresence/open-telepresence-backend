use tracing_subscriber::{prelude::*, fmt, filter};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

pub fn start_logger() {
    let stdout_filter = filter::FilterFn::new(|metadata| {
        metadata.target() == "signal_server"
    });
    let file_filter = filter::FilterFn::new(|metadata| {
        metadata.target() == "signal_server"
    });

    let stdout_logger = fmt::layer()
        .with_target(false)
        .with_line_number(true)
        .with_file(true)
        .with_filter(stdout_filter);

    
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "signal_server.log");
    let debug_log = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_line_number(true)
        .with_file(true)
        .with_writer(file_appender)
        .with_filter(file_filter);

    tracing_subscriber::registry()
        .with(
            stdout_logger
                .and_then(debug_log)
        )
        .init();
}
