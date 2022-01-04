use crate::allocator::{ENABLE_STACK_TRACE, REPORT_USAGE_INTERVAL};
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
    pub fn set_report_usage_interval(self, value: usize) -> Self {
        REPORT_USAGE_INTERVAL.store(value, Ordering::Relaxed);
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::AllocatorConfig;

    #[test]
    fn test() {
        let _ = AllocatorConfig::default()
            .set_report_usage_interval(10000000)
            .enable_stack_trace(false);
    }
}
