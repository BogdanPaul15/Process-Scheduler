use std::num::NonZeroUsize;

use crate::{Pid, Process, ProcessState, Scheduler, Syscall, SyscallResult};

pub struct ProcessInfo {
    pid: Pid,
    state: ProcessState,
    timings: (usize, usize, usize),
    priority: i8,
    extra: String,
}

pub struct RoundRobin {
    timeslice: NonZeroUsize,
    minimum_remaining_timeslice: usize,
    all: Vec<ProcessInfo>,
    ready: Vec<ProcessInfo>,
    wait: Vec<ProcessInfo>,
    pid_counter: usize,
    running_process: Option<ProcessInfo>,
    remaining_running_time: usize,
    init: bool,
}
impl RoundRobin {
    pub fn new(timeslice: NonZeroUsize, minimum_remaining_timeslice: usize) -> Self {
        Self {
            timeslice,
            minimum_remaining_timeslice,
            all: Vec::new(),
            ready: Vec::new(),
            wait: Vec::new(),
            pid_counter: 1,
            running_process: None,
            remaining_running_time: 0,
            init: false,
        }
    }
    pub fn generate_pid(&mut self) -> Pid {
        // Generate a new PID
        let new_pid = Pid::new(self.pid_counter);
        self.pid_counter += 1;
        new_pid
    }
}

impl Process for ProcessInfo {
    fn pid(&self) -> crate::Pid {
        self.pid
    }
    fn state(&self) -> ProcessState {
        self.state
    }
    fn timings(&self) -> (usize, usize, usize) {
        self.timings
    }
    fn priority(&self) -> i8 {
        self.priority
    }
    fn extra(&self) -> String {
        String::new()
    }
}

impl Scheduler for RoundRobin {
    fn next(&mut self) -> crate::SchedulingDecision {
        // Check if there is a running process
        match self.running_process.take() {
            Some(mut running_process) => {
                // If a running process exists
                if self.remaining_running_time <= self.minimum_remaining_timeslice {
                    // Mark the current running process as Ready and push it to the ready queue
                    running_process.state = ProcessState::Ready;
                    self.ready.push(running_process);
                    // Get the next process in the ready queue and mark it as running
                    if !self.ready.is_empty() {
                        let mut proc = self.ready.remove(0);
                        proc.state = ProcessState::Running;
                        self.running_process = Some(proc);
                        return crate::SchedulingDecision::Run {
                            pid: self.running_process.as_ref().unwrap().pid,
                            timeslice: self.timeslice,
                        };
                    } else {
                        return crate::SchedulingDecision::Deadlock;
                    }
                } else {
                    return crate::SchedulingDecision::Run {
                        pid: self.running_process.as_ref().unwrap().pid(),
                        timeslice: self.timeslice,
                    };
                }
            }
            None => {
                // There is no running process (primul fork, exit, toate wait sau sleep)
                if !self.ready.is_empty() {
                    if self.init {
                        return crate::SchedulingDecision::Panic;
                    }
                    let mut proc = self.ready.remove(0);
                    proc.state = ProcessState::Running;
                    self.running_process = Some(proc);
                    return crate::SchedulingDecision::Run {
                        pid: self.running_process.as_ref().unwrap().pid(),
                        timeslice: self.timeslice,
                    };
                } else {
                    if !self.wait.is_empty() {
                        // verifica deadlock
                        let mut is_deadlock = true;
                        for proc in &self.wait {
                            if let ProcessState::Waiting { event: None } = &proc.state {
                                is_deadlock = false;
                                break;
                            }
                        }
                        if is_deadlock {
                            return crate::SchedulingDecision::Deadlock;
                        } else {
                            // sleep for minimum time
                        }
                        return crate::SchedulingDecision::Done;
                    }
                    // Handle the case when there's no process available to run
                    return crate::SchedulingDecision::Done;
                }
            }
        }
    }

    fn stop(&mut self, _reason: crate::StopReason) -> crate::SyscallResult {
        match _reason {
            crate::StopReason::Syscall { syscall, remaining } => match syscall {
                Syscall::Fork(priority) => {
                    let new_pid = self.generate_pid();
                    let new_process = ProcessInfo {
                        pid: new_pid,
                        state: ProcessState::Ready,
                        timings: (0, 0, 0),
                        priority,
                        extra: String::new(),
                    };
                    self.ready.push(new_process);
                    if let Some(mut running_process) = self.running_process.take() {
                        // Update the timings
                        running_process.timings.0 += usize::from(self.timeslice) - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += usize::from(self.timeslice) - remaining;
                        self.remaining_running_time = remaining;
                    }
                    // Save the remaining time for the running process
                    SyscallResult::Pid(new_pid)
                }
                Syscall::Sleep(_amount) => SyscallResult::Success,
                Syscall::Wait(_event) => SyscallResult::Success,
                Syscall::Signal(_event) => SyscallResult::Success,
                Syscall::Exit => {
                    // if self.running_process.pid
                    // self.running_process = None;
                    // // update all processes
                    // SyscallResult::Success
                    // fa update la timings si syscall
                    if let Some(running_process) = self.running_process.take() {
                        if running_process.pid == 1 {
                            self.init = true;
                        }
                    }
                    self.running_process = None;
                    SyscallResult::Success
                }
            },
            crate::StopReason::Expired => {
                if let Some(mut running_process) = self.running_process.take() {
                    // Change its state and update the timings
                    running_process.state = ProcessState::Ready;
                    running_process.timings.0 += usize::from(self.timeslice);
                    running_process.timings.2 += usize::from(self.timeslice);
                    // Push to the ready queue
                    self.ready.push(running_process);
                }
                SyscallResult::Success
            }
        }
    }

    fn list(&mut self) -> Vec<&dyn Process> {
        let mut list: Vec<&dyn Process> = Vec::new();
        for i in &self.ready {
            list.push(i)
        }
        if let Some(x) = &self.running_process {
            list.push(x);
        }
        list
    }
}
