// Observability layer with zero-cost abstractions
// Only compiled when 'production' features are enabled

/// Conditional tracing macro - zero cost in benchmark mode
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! trace_exec {
    ($($arg:tt)*) => {
        tracing::debug!($($arg)*);
    };
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! trace_exec {
    ($($arg:tt)*) => {};
}

/// Conditional info logging
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! info_exec {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*);
    };
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! info_exec {
    ($($arg:tt)*) => {};
}

/// Conditional metrics recording
#[cfg(feature = "metrics")]
#[macro_export]
macro_rules! record_metric {
    (counter, $name:expr) => {
        $crate::observability::METRICS.record_counter($name);
    };
    (histogram, $name:expr, $value:expr) => {
        $crate::observability::METRICS.record_histogram($name, $value);
    };
}

#[cfg(not(feature = "metrics"))]
#[macro_export]
macro_rules! record_metric {
    ($($arg:tt)*) => {};
}

#[cfg(feature = "metrics")]
pub mod metrics {
    use prometheus::{IntCounter, Histogram, Registry, HistogramOpts, Opts};
    use std::sync::OnceLock;
    
    pub struct Metrics {
        pub tx_total: IntCounter,
        pub tx_success: IntCounter,
        pub tx_revert: IntCounter,
        pub exec_time: Histogram,
        pub gas_used: Histogram,
        pub registry: Registry,
    }
    
    impl Metrics {
        pub fn new() -> Self {
            let registry = Registry::new();
            
            let tx_total = IntCounter::with_opts(
                Opts::new("transactions_total", "Total transactions executed")
            ).unwrap();
            
            let tx_success = IntCounter::with_opts(
                Opts::new("transactions_success", "Successful transactions")
            ).unwrap();
            
            let tx_revert = IntCounter::with_opts(
                Opts::new("transactions_revert", "Reverted transactions")
            ).unwrap();
            
            let exec_time = Histogram::with_opts(
                HistogramOpts::new("execution_time_seconds", "Transaction execution time")
            ).unwrap();
            
            let gas_used = Histogram::with_opts(
                HistogramOpts::new("gas_used", "Gas used per transaction")
            ).unwrap();
            
            registry.register(Box::new(tx_total.clone())).unwrap();
            registry.register(Box::new(tx_success.clone())).unwrap();
            registry.register(Box::new(tx_revert.clone())).unwrap();
            registry.register(Box::new(exec_time.clone())).unwrap();
            registry.register(Box::new(gas_used.clone())).unwrap();
            
            Self {
                tx_total,
                tx_success,
                tx_revert,
                exec_time,
                gas_used,
                registry,
            }
        }
        
        pub fn record_counter(&self, name: &str) {
            match name {
                "tx_total" => self.tx_total.inc(),
                "tx_success" => self.tx_success.inc(),
                "tx_revert" => self.tx_revert.inc(),
                _ => {}
            }
        }
        
        pub fn record_histogram(&self, name: &str, value: f64) {
            match name {
                "exec_time" => self.exec_time.observe(value),
                "gas_used" => self.gas_used.observe(value),
                _ => {}
            }
        }
    }
    
    pub static METRICS: OnceLock<Metrics> = OnceLock::new();
    
    pub fn init_metrics() -> &'static Metrics {
        METRICS.get_or_init(|| Metrics::new())
    }
}

#[cfg(feature = "logging")]
pub fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    use tracing_appender::non_blocking;
    
    // Non-blocking async logging for minimal overhead
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    
    fmt()
        .with_writer(non_blocking)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .json()
        .init();
    
    // Leak guard to keep it alive for program duration
    std::mem::forget(_guard);
}

#[cfg(not(feature = "logging"))]
pub fn init_tracing() {
    // No-op in benchmark mode
}
