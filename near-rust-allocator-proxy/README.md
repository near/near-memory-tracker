Track Rust memory usage by adding a 32 bytes header to all allocations.
See https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html

# Usage
You can use code below to enable usage of this library:
```rust
use near_rust_allocator_proxy::allocator::MyAllocator;

#[global_allocator]
static ALLOC: MyAllocator = MyAllocator;
```

# Design
* header - For each memory allocation we add a 32 bytes header. This allows figuring out how memory was allocated by looking at memory dump of the process.
* per thread memory usage stats - `thread_memory_usage(tid)` method can be used to get amount of memory allocated by thread
* `PRINT_STACK_TRACE_ON_MEMORY_SPIKE` - if set to true a stack trace will be used on memory spike

# Constants
* `ENABLE_STACK_TRACE` - if enabled `backtrace` will get executed on each allocation and stack pointer will be added to the header
* `MIN_BLOCK_SIZE` - if allocation size of below `MIN_BLOCK_SIZE`, we will only run `backtrace` `SMALL_BLOCK_TRACE_PROBABILITY` percentage of time
* `SMALL_BLOCK_TRACE_PROBABILITY` - probability of running a stack trace for small allocations
* `REPORT_USAGE_INTERVAL` - if printing memory spikes is enabled print if memory usage exceeded this value in bytes
* `PRINT_STACK_TRACE_ON_MEMORY_SPIKE` - if true print stack trace when memory usage exceeds `REPORT_USAGE_INTERVAL` on given Rust thread

# Header representation

Allocation structure:
* magic - unique 8 bytes identifier, which is used to mark memory allocations
* size - size in bytes
* tid - thread id
* stack - stack trace during time of allocation

```rust
#[repr(C)]
struct AllocHeader {
    magic: u64,
    size: u64,
    tid: u64,
    stack: [*mut c_void; STACK_SIZE],
}
```

# TODO
* Add methods to set the configuration instead of having to change the constants.
* Add tests
