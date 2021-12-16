mod allocator;

pub use allocator::{
    current_thread_memory_usage, current_thread_peak_memory_usage, print_counters_ary,
    reset_memory_usage_max, thread_memory_count, thread_memory_usage, total_memory_usage,
    MyAllocator,
};
