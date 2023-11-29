use std::fmt::{self, Display};
use std::num::NonZeroUsize;
use std::ops::Add;

/// The PID of a process
///
/// The PID cannot be 0, PIDs start from 1.
#[derive(PartialEq, Eq, Copy, Clone, Hash, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Pid(NonZeroUsize);

impl Pid {
    pub fn new(pid: usize) -> Pid {
        Pid(NonZeroUsize::new(pid).unwrap())
    }
}

impl PartialEq<usize> for Pid {
    fn eq(&self, other: &usize) -> bool {
        self.0.get() == *other
    }
}

impl Display for Pid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Pid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add<usize> for Pid {
    type Output = Pid;

    fn add(self, rhs: usize) -> Self::Output {
        Pid::new(self.0.get() + rhs)
    }
}

/// The action that the scheduler asks the OS to take.
///
/// This is returned by the [`Scheduler::next`] function.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SchedulingDecision {
    /// Run the process with PID `pid` for a maximum of `timeslice` time units.
    Run { pid: Pid, timeslice: NonZeroUsize },
    /// Sleep the amount of specified time units.
    Sleep(NonZeroUsize),
    /// The OS cannot continue anymore, as all the processes are waiting for events.
    ///
    /// In this case there is no other process that can fie any events, which means
    /// that all the processes will wait indefinitely.
    Deadlock,

    /// The process with PID 1 has stopped.
    Panic,

    /// There are no more processes to schedule.
    Done,
}

impl Display for SchedulingDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchedulingDecision::Run { pid, timeslice } => {
                write!(f, "Run {} for {} slices", pid, timeslice)
            }
            SchedulingDecision::Sleep(amount) => {
                write!(f, "Sleep for {} slices", amount)
            }
            SchedulingDecision::Deadlock => {
                write!(f, "Deadlock, unable to schedule anymore processes")
            }
            SchedulingDecision::Panic => {
                write!(f, "Panic, process 1 has stopped")
            }
            SchedulingDecision::Done => {
                write!(f, "Done, no more processes")
            }
        }
    }
}

/// A system call that processes make towards the scheduler.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Syscall {
    /// Create a new process and return its PID.
    Fork(
        /// The process's priority. Some scheduling algorithms can ignore this value.
        i8,
    ),

    /// Ask the scheduler to suspend for an amount of time
    Sleep(
        /// The amount of time that the process should sleep. The process
        /// will be placed in the [`ProcessState::Waiting`] state for this
        /// amount of time.
        usize,
    ),

    /// Wait for an event
    Wait(
        /// The event number. The process will be placed in the [`ProcessState::Waiting`]
        /// until another process issues a [`Syscall::Signal`] system call with this
        /// event number.
        usize,
    ),

    /// Signal all processes that wait for an event.
    Signal(
        /// The event number. All processes that are waiting for this event
        /// will be woken up and placed in the [`ProcessState::Ready`] state.
        usize,
    ),

    /// Ask the scheduler to finish the process.
    ///
    /// The process will never be scheduled again and will be deleted
    /// from the list of processes the the scheduler keeps track of.
    Exit,
}

/*
///
/// If all the processes are in the sleep state, the scheduler will return
/// the minimum value of the sleeping times.
 */

/// The result returned by a system call.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SyscallResult {
    /// Returned after a [`Syscall::Fork`] system call.
    Pid(
        /// The PID of the new process.
        Pid,
    ),
    /// The system call was successful.
    ///
    /// This is the value returned by most system calls.
    Success,

    /// The system call was issues while no process was scheduled.
    NoRunningProcess,
}

/// The reason that a process has stopped and the OS
/// has called the scheduler.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum StopReason {
    /// The process sent a [`Syscall`] system call.
    Syscall {
        /// The system call.
        syscall: Syscall,

        /// The number of time units that the process has not used from its quanta
        remaining: usize,
    },

    /// The timeslice allocated for the process has expired and the process
    /// has been preempted.
    Expired,
}

impl Display for StopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StopReason::Syscall { syscall, remaining } => {
                write!(f, "Syscall {syscall:?}, remaining {remaining}")
            }
            StopReason::Expired => {
                write!(f, "Expired")
            }
        }
    }
}

impl StopReason {
    pub fn syscall(syscall: Syscall) -> StopReason {
        StopReason::Syscall {
            syscall,
            remaining: 0,
        }
    }

    pub fn set_remaining(&mut self, remaining: usize) {
        if let StopReason::Syscall { syscall, .. } = *self {
            *self = StopReason::Syscall { syscall, remaining };
        }
    }

    pub fn expired() -> StopReason {
        StopReason::Expired
    }
}

/// The trait that any scheduler has to implement.
pub trait Scheduler: Send {
    /// Returns the action that the OS has to perform next.
    fn next(&mut self) -> SchedulingDecision;

    /// The scheduler is informed about the stopping of a process
    /// and the reason.
    fn stop(&mut self, reason: StopReason) -> SyscallResult;

    /// Returns the list of processes.
    fn list(&mut self) -> Vec<&dyn Process>;
}

/// The state of a process.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ProcessState {
    /// The process is ready to be scheduled.
    Ready,

    /// The process is currently scheduled.
    Running,

    /// The process is waiting for an event or sleeping.
    Waiting {
        /// The event that the process is waiting for.
        ///
        /// If the event is [`None`], the process is sleeping.
        event: Option<usize>,
    },
}

impl Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessState::Ready => write!(f, "READY"),
            ProcessState::Running => write!(f, "RUNNING"),
            ProcessState::Waiting { event } => {
                if let Some(event) = event {
                    write!(f, "EVENT {}", event)
                } else {
                    write!(f, "SLEEP")
                }
            }
        }
    }
}

/// The trait that the Process Control Block (PCB) has to implement.
///
/// The PCB can be implemented with any data structure as long as
/// it implements this trait.
pub trait Process: Send {
    /// Return the PID of the process.
    fn pid(&self) -> Pid;

    /// Return the state of the process.
    fn state(&self) -> ProcessState;

    /// Returns process timings as a tuple of (total, syscalls, execution)
    fn timings(&self) -> (usize, usize, usize);

    /// Returns the process priority
    fn priority(&self) -> i8;

    /// Returns details information
    fn extra(&self) -> String;
}
