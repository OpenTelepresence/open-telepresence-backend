use tracing::Subscriber;
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::Layer;
use tracing_subscriber::prelude::*;


pub struct StdoutLogger;

impl<S: Subscriber> Layer<S> for StdoutLogger {
}

pub fn start_logger() {
  // tracing::subscriber::set_global_default(file_logger)
  //   .expect("setting file_logger failed");
  let fmt_layer = fmt::layer()
    .with_target(false)
    .with_line_number(true)
    .with_file(true);

  let filter_layer = EnvFilter::try_from_default_env()
    .or_else(|_| EnvFilter::try_new("debug"))
    .unwrap();

  let stdout_logger = StdoutLogger;
  tracing_subscriber::registry()
    .with(filter_layer)
    .with(fmt_layer)
    .with(stdout_logger)
    .init();
}
