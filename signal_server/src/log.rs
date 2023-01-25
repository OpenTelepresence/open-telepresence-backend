use tracing_subscriber::{prelude::*, fmt, EnvFilter};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

pub fn start_logger() {

    let stdout_logger = fmt::layer()
        .with_target(false)
        .with_line_number(true)
        .with_file(true);

    let stdout_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("debug"))
        .unwrap();

    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "signal_server.log");
    let debug_log = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_line_number(true)
        .with_file(true)
        .with_writer(file_appender);

    tracing_subscriber::registry()
        .with(
            stdout_logger
                .with_filter(stdout_filter)
                .and_then(debug_log)
        )
        .init();
}
