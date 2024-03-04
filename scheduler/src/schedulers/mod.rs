//! Implement the schedulers in this module
//!
//! You might want to create separate files
//! for each scheduler and export it here
//! like
//!
//! ```ignore
//! mod scheduler_name
//! pub use scheduler_name::SchedulerName;
//! ```
//!

mod empty;
pub use empty::Empty;

mod round_robin;
pub use round_robin::RoundRobin;

mod round_robin_priority;
pub use round_robin_priority::RoundRobinPriority;
