//! A processor simulation library
//!
//! This is used for simulating scheduler from the [`scheduler`] crate.

use std::collections::HashMap;
use std::fmt::{self, Display};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::{mem, thread};

use scheduler::{
    Pid, ProcessState, Scheduler, SchedulingDecision, StopReason, Syscall, SyscallResult,
};

/// Running iteration log
#[derive(Debug)]
pub struct Log {
    /// The action requested by the scheduler.
    pub decision: SchedulingDecision,

    /// The reason that a process has stopped.
    pub stop_reason: Option<(StopReason, SyscallResult)>,

    /// The list of processes and their corresponding states
    /// returned by the scheduler.
    pub processes: HashMap<Pid, ProcessInfo>,
}

impl Log {
    fn new(
        decision: SchedulingDecision,
        stop_reason: Option<(StopReason, SyscallResult)>,
        processes: HashMap<Pid, ProcessInfo>,
    ) -> Log {
        Log {
            decision,
            stop_reason,
            processes,
        }
    }
}

impl Display for Log {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.decision).unwrap();
        // writeln!(f, "===== Processes =====");
        writeln!(f, "PID\tSTATE\t\tPRI\tTOTAL\tSYSCALL\tEXECUTE\tEXTRA").unwrap();
        let mut pids = self.processes.keys().collect::<Vec<&Pid>>();
        pids.sort();
        for pid in pids.into_iter() {
            writeln!(f, "{}", self.processes.get(pid).unwrap()).unwrap();
        }
        if let Some(log) = self.stop_reason {
            writeln!(f, "{} -> {:?}", log.0, (log.1)).unwrap();
        }
        writeln!(f)
    }
}

impl PartialEq<Log> for Log {
    fn eq(&self, other: &Log) -> bool {
        self.decision == other.decision
            && self.stop_reason == other.stop_reason
            && self.processes == other.processes
    }
}

/// Information about a process state.
#[derive(Debug, PartialEq)]
pub struct ProcessInfo {
    /// The PID of the process.
    pub pid: Pid,

    /// The process state.
    pub state: ProcessState,

    /// The process timings (total time, system call time, running time).
    pub timings: (usize, usize, usize),

    /// The process priority
    pub priority: i8,

    /// Extra details about the process
    pub extra: String,
}

impl ProcessInfo {
    fn new(
        pid: Pid,
        state: ProcessState,
        timings: (usize, usize, usize),
        priority: i8,
        extra: String,
    ) -> ProcessInfo {
        ProcessInfo {
            pid,
            state,
            timings,
            priority,
            extra,
        }
    }
}

impl Display for ProcessInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\t{}\t\t{}\t{}\t{}\t{}\t{}",
            self.pid,
            self.state,
            self.priority,
            self.timings.0,
            self.timings.1,
            self.timings.2,
            self.extra
        )
    }
}

/// The processor simulator.
pub struct Processor<S: Scheduler + 'static> {
    scheduler: Arc<Mutex<S>>,
    current_process: Arc<(Mutex<Option<Pid>>, Condvar)>,
    remaining: AtomicUsize,
    logs: Mutex<Vec<Log>>,
    running: AtomicBool,
}

impl<S: Scheduler + 'static> Processor<S> {
    /// Start a new processor simulation.
    ///
    /// * `scheduler` - the scheduler to use for the simulation.
    /// * `f` - a function with the instructions for the process with
    ///         PID 1.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use processor::Processor;
    /// use std::num::NonZeroUsize;
    ///
    /// Processor::run(scheduler::round_robin(NonZeroUsize::new(2).unwrap(), 1), |process| {
    ///     process.exec();
    ///     process.exec();
    ///     process.exec();
    ///     process.exec();
    ///     process.fork(
    ///         |process| {
    ///             process.exec();
    ///             process.exec();
    ///             process.wait(1);
    ///         },
    ///         0,
    ///     );
    ///     process.sleep(10);
    ///     process.signal(1);
    ///     process.exec();
    /// });
    /// ```
    pub fn run<F>(scheduler: S, f: F) -> Vec<Log>
    where
        F: FnOnce(&Process<S>) + Send,
    {
        let processor = Arc::new(Processor {
            scheduler: Arc::new(Mutex::new(scheduler)),
            current_process: Arc::new((Mutex::new(None), Condvar::new())),
            remaining: AtomicUsize::new(1),
            logs: Mutex::new(vec![]),
            running: AtomicBool::new(true),
        });

        let SyscallResult::Pid(pid) = processor.scheduler(StopReason::syscall(Syscall::Fork(0))) else {
            panic!("Fork did not return a pid");
        };

        if pid != 1 {
            panic!("Scheduler did not return PID 1 for the first process");
        }

        let mutex = processor.current_process.clone();
        thread::scope(|s| {
            s.spawn(move || {
                let process = Process {
                    pid,
                    mutex,
                    processor,
                };
                process.suspend();
                f(&process);
                process.exit();
                process.processor.get_logs()
            })
            .join()
            .unwrap()
        })
    }

    fn exec(&self) -> bool {
        if self.is_running() {
            self.remaining.fetch_sub(1, Ordering::Relaxed);
            self.remaining.load(Ordering::Relaxed) != 0
        } else {
            true
        }
    }

    fn scheduler(&self, mut reason: StopReason) -> SyscallResult {
        if self.is_running() {
            self.remaining.fetch_sub(1, Ordering::Relaxed);
            let mut scheduler = self.scheduler.lock().unwrap();
            reason.set_remaining(self.remaining.load(Ordering::Relaxed));
            let result = scheduler.stop(reason);
            {
                let mut logs = self.logs.lock().unwrap();
                let len = logs.len();
                if len > 0 {
                    if let Some(log) = logs.get_mut(len - 1) {
                        log.stop_reason = Some((reason, result));
                    };
                }
            }

            let mut current_process = self.current_process.0.lock().unwrap();
            *current_process = None;
            while self.is_running() && current_process.is_none() {
                let next = scheduler.next();
                let mut process_map = HashMap::new();
                for process in scheduler.list() {
                    process_map.insert(
                        process.pid(),
                        ProcessInfo::new(
                            process.pid(),
                            process.state(),
                            process.timings(),
                            process.priority(),
                            process.extra(),
                        ),
                    );
                }
                (*self.logs.lock().unwrap()).push(Log::new(next, None, process_map));
                // println!("{}", next);
                match next {
                    SchedulingDecision::Run { pid, timeslice } => {
                        self.remaining.store(timeslice.into(), Ordering::Relaxed);
                        *current_process = Some(pid);
                        self.current_process.1.notify_all();
                    }
                    SchedulingDecision::Sleep(time) => {
                        println!("SLEEP {time}");
                    }
                    SchedulingDecision::Deadlock => {
                        println!("DEADLOCK");
                        self.stop();
                    }
                    SchedulingDecision::Panic => {
                        println!("PANIC");
                        self.stop();
                    }
                    SchedulingDecision::Done => {
                        println!("DONE");
                        self.stop();
                    }
                }
            }
            result
        } else {
            SyscallResult::NoRunningProcess
        }
    }

    fn get_logs(&self) -> Vec<Log> {
        let mut logs = self.logs.lock().unwrap();
        let mut res = vec![];
        mem::swap(&mut res, &mut *logs);
        res
    }

    fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.current_process.1.notify_all();
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

/// The interface offered by the [`Processor`] to a [`Process`].
pub struct Process<S: Scheduler + 'static> {
    /// The PID of the process.
    pub pid: Pid,
    processor: Arc<Processor<S>>,
    mutex: Arc<(Mutex<Option<Pid>>, Condvar)>,
}

impl<S: Scheduler + 'static> Process<S> {
    fn suspend(&self) {
        let mut wait = self.mutex.0.lock().unwrap();
        while self.processor.is_running() && *wait != Some(self.pid) {
            // println!("SUSPENDED {}", self.pid);
            wait = self.mutex.1.wait(wait).unwrap();
        }
        if self.processor.is_running() {
            println!("RUNNING {}", self.pid);
        }
    }

    /// Execute one unit of time.
    pub fn exec(&self) {
        println!("{}: EXEC", self.pid);
        if !self.processor.exec() {
            println!("PREEMPTED {}", self.pid);
            self.processor.scheduler(StopReason::expired());
            self.suspend();
        }
    }

    /// Send a [`Syscall::Fork`] system call.
    pub fn fork<F>(&self, f: F, priority: i8) -> Pid
    where
        F: FnOnce(&Process<S>) + Send + 'static,
    {
        let SyscallResult::Pid(pid) = self.processor.scheduler(StopReason::syscall(Syscall::Fork(priority))) else {
            panic!("Fork did not return a pid");
        };

        println!("{}: FORK {}", self.pid, pid);

        let mutex = self.mutex.clone();
        let processor = self.processor.clone();

        thread::spawn(move || {
            let process = Process {
                pid,
                mutex,
                processor,
            };
            process.suspend();
            f(&process);
            process.exit();
        });
        self.suspend();
        pid
    }

    /// Send a [`Syscall::Wait`] system call.
    ///
    /// * `event` - the event number to wait for.
    pub fn wait(&self, event: usize) {
        println!("{}: WAIT {}", self.pid, event);
        self.processor
            .scheduler(StopReason::syscall(Syscall::Wait(event)));
        self.suspend();
    }

    /// Send a [`Syscall::Signal`] system call.
    ///
    /// * `event` - the event number to signal.
    pub fn signal(&self, event: usize) {
        println!("{}: SIGNAL {}", self.pid, event);
        self.processor
            .scheduler(StopReason::syscall(Syscall::Signal(event)));
        self.suspend();
    }

    /// Send a [`Syscall::Sleep`] system call.
    ///
    /// * `timeslice` - the amout of time to sleep.
    pub fn sleep(&self, timeslice: usize) {
        println!("{}: SLEEP {}", self.pid, timeslice);
        self.processor
            .scheduler(StopReason::syscall(Syscall::Sleep(timeslice)));
        self.suspend();
    }

    fn exit(&self) {
        println!("{}: EXIT", self.pid);
        self.processor.scheduler(StopReason::syscall(Syscall::Exit));
    }
}

/// Format the [`Processor`]'s logs to a [`String`].
///
/// * `logs` - the logs returned by the [`Processor`].
///
/// ## Example
///
/// ```rust
/// use processor::Processor;
/// use std::num::NonZeroUsize;
/// use processor::format_logs;
///
/// let logs = Processor::run(scheduler::round_robin(NonZeroUsize::new(2).unwrap(), 1), |process| {
///     /* ... */
/// });
///
/// println!("{}", format_logs(&logs));
/// ```
pub fn format_logs(logs: &[Log]) -> String {
    let mut s = String::new();
    for (iteration, log) in logs.iter().enumerate() {
        fmt::write(
            &mut s,
            format_args!("===== Iteration: {} =====\n{}\n", iteration + 1, log),
        )
        .unwrap();
    }
    s
}
