[![Review Assignment Due Date](https://classroom.github.com/assets/deadline-readme-button-24ddc0f5d75046c5622901739e7c5dd533143b0c8e959d652212380cedb1ea36.svg)](https://classroom.github.com/a/2eN9hsMw)
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
