mod allocator;
mod config;

pub use allocator::{
    current_thread_memory_usage, current_thread_peak_memory_usage, print_memory_stats,
    reset_memory_usage_max, thread_memory_count, thread_memory_usage, total_memory_usage,
    AllocHeader, ProxyAllocator, ALLOC_HEADER_SIZE,
};

pub use config::AllocatorConfig;
