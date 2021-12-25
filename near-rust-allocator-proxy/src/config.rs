use crate::allocator::{
    ENABLE_STACK_TRACE, LOGS_PATH, REPORT_USAGE_INTERVAL, SAVE_STACK_TRACES_TO_FILE,
};
use std::sync::atomic::Ordering;

#[derive(Default)]
pub struct AllocatorConfig {}

impl AllocatorConfig {
    /// Enable calling `backtrace` to fill out data
    pub fn enable_stack_trace(self, value: bool) -> Self {
        ENABLE_STACK_TRACE.store(value, Ordering::Relaxed);
        self
    }

    /// Save stack tres to file
    pub fn save_stack_traces_to_file(self, value: bool) -> Self {
        SAVE_STACK_TRACES_TO_FILE.store(value, Ordering::Relaxed);
        self
    }

    /// Save stack tres to file
    pub fn set_report_usage_interval(self, value: usize) -> Self {
        REPORT_USAGE_INTERVAL.store(value, Ordering::Relaxed);
        self
    }

    /// Set file where to write stack traces
    pub fn set_traces_file(self, _file_path: &str) -> Self {
        *LOGS_PATH.lock().unwrap() = _file_path.to_string();
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::AllocatorConfig;

    #[test]
    fn test() {
        let _ = AllocatorConfig::default()
            .save_stack_traces_to_file(true)
            .set_report_usage_interval(10000000)
            .enable_stack_trace(false);
    }
}
