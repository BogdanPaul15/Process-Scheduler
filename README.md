# Process Scheduler

## Getting started

Please run `cargo doc --open` to create and open the documentation.

Your job is:
1. Implement the schedulers in the `scheduler` crate in the folder `scheduler/src/schedulers`.
2. Export the scheduler in the `scheduler/src/lib.rs` file using the three functions
   - `round_robin(...)`
   - `priority_queue(...)`
   - `cfs(...)`
3. Test them using the `runner` crate by using them in `runner/src/main.rs`.
