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
    // all: Vec<ProcessInfo>,
    ready: Vec<ProcessInfo>,
    wait: Vec<ProcessInfo>,
    pid_counter: usize,
    running_process: Option<ProcessInfo>,
    remaining_running_time: usize,
    init: bool,
    sleep_amounts: Vec<usize>,
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
            sleep_amounts: Vec::new(),
        }
    }
    pub fn generate_pid(&mut self) -> Pid {
        // Generate a new PID
        let new_pid = Pid::new(self.pid_counter);
        self.pid_counter += 1;
        new_pid
    }
    pub fn increase_timings(&mut self, amount: usize) {
        for proc in &mut self.ready {
            proc.timings.0 += amount;
        }
        for proc in &mut self.wait {
            proc.timings.0 += amount;
        }
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
                // Check if the running process still can run
                if self.remaining_running_time < self.minimum_remaining_timeslice {
                    // If it cant run anymore, mark it as Ready and send it to the ready queue
                    running_process.state = ProcessState::Ready;
                    self.ready.push(running_process);
                    // Get the first process from the ready queue and mark it as running
                    if !self.ready.is_empty() {
                        let mut proc = self.ready.remove(0);
                        proc.state = ProcessState::Running;
                        self.running_process = Some(proc);
                        self.remaining_running_time = self.timeslice.into();
                        // Return its pid
                        return crate::SchedulingDecision::Run {
                            pid: self.running_process.as_ref().unwrap().pid(),
                            timeslice: NonZeroUsize::new(self.remaining_running_time).unwrap(),
                        };
                    } else {
                        // Check for deadlock
                        return crate::SchedulingDecision::Deadlock;
                    }
                } else {
                    self.running_process = Some(running_process);
                    // Reschedule the running process again
                    return crate::SchedulingDecision::Run {
                        pid: self.running_process.as_ref().unwrap().pid(),
                        timeslice: NonZeroUsize::new(self.remaining_running_time).unwrap(),
                    };
                }
            }
            None => {
                // There is no running process
                if !self.ready.is_empty() {
                    // Check if the process with pid 1 has exited
                    if self.init {
                        self.init = false;
                        return crate::SchedulingDecision::Panic;
                    }
                    // Return the first process from the ready queue
                    let mut proc = self.ready.remove(0);
                    proc.state = ProcessState::Running;
                    self.running_process = Some(proc);
                    return crate::SchedulingDecision::Run {
                        pid: self.running_process.as_ref().unwrap().pid(),
                        timeslice: self.timeslice,
                    };
                } else {
                    if !self.wait.is_empty() {
                        // Check for deadlock
                        let mut is_deadlock = true;
                        for proc in &self.wait {
                            if let ProcessState::Waiting { event } = &proc.state {
                                if *event == None {
                                    is_deadlock = false;
                                    break;
                                }
                            }
                        }
                        if is_deadlock {
                            return crate::SchedulingDecision::Deadlock;
                        } else {
                            // let mut min_amount = std::usize::MAX;
                            // for &amount in &self.sleep_amounts {
                            //     if amount < min_amount {
                            //         min_amount = amount;
                            //     }
                            // }
                            // if let Some(min_nonzero_amount) = NonZeroUsize::new(min_amount) {
                            //     return crate::SchedulingDecision::Sleep(min_nonzero_amount);
                            // }
                            return crate::SchedulingDecision::Done;
                        }
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
                    // Increase all total timings
                    self.increase_timings(usize::from(self.timeslice) - remaining);
                    // Generate a new process
                    let new_pid = self.generate_pid();
                    let new_process = ProcessInfo {
                        pid: new_pid,
                        state: ProcessState::Ready,
                        timings: (0, 0, 0),
                        priority,
                        extra: String::new(),
                    };
                    // Add it to the ready queue
                    self.ready.push(new_process);
                    if let Some(mut running_process) = self.running_process.take() {
                        // Update the timings of the running process
                        running_process.timings.0 += usize::from(self.timeslice) - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += usize::from(self.timeslice) - remaining - 1;
                        // Save the remaining time for the running process
                        self.remaining_running_time = remaining;
                        self.running_process = Some(running_process);
                    }
                    // Return the pid of the just created process
                    SyscallResult::Pid(new_pid)
                }
                Syscall::Sleep(amount) => {
                    if let Some(mut running_process) = self.running_process.take() {
                        running_process.state = ProcessState::Waiting { event: None };
                        running_process.timings.0 += usize::from(self.timeslice) - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += usize::from(self.timeslice) - remaining - 1;
                        self.increase_timings(usize::from(self.timeslice) - remaining);
                        self.wait.push(running_process);
                        // Convert the amount to NonZeroUsize and push it to the sleep_amounts vector
                        self.sleep_amounts.push(amount);
                    }
                    self.running_process = None;
                    SyscallResult::Success
                }
                Syscall::Wait(e) => {
                    // increase all timings
                    if let Some(mut running_process) = self.running_process.take() {
                        running_process.state = ProcessState::Waiting { event: (Some(e)) };
                        running_process.timings.0 += usize::from(self.timeslice) - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += usize::from(self.timeslice) - remaining;
                        self.increase_timings(usize::from(self.timeslice) - remaining);
                        self.wait.push(running_process);
                    }
                    self.running_process = None;
                    SyscallResult::Success
                }
                Syscall::Signal(e) => {
                    // for proc in &mut self.wait {
                    //     if let ProcessState::Waiting { event } = &proc.state {
                    //         if *event == Some(e) {
                    //             proc.state = ProcessState::Ready;
                    //             let new_proc = ProcessInfo {
                    //                 pid: proc.pid,
                    //                 state: ProcessState::Ready,
                    //                 timings: proc.timings,
                    //                 priority: proc.priority,
                    //                 extra: proc.extra.clone(),
                    //             };
                    //             self.ready.push(new_proc);
                    //             self.wait.remove(new_proc);
                    //         }
                    //     }
                    // }
                    self.wait.retain(|proc| {
                        if let ProcessState::Waiting { event } = &proc.state {
                            if *event == Some(e) {
                                let new_proc = ProcessInfo {
                                    pid: proc.pid,
                                    state: ProcessState::Ready,
                                    timings: proc.timings,
                                    priority: proc.priority,
                                    extra: proc.extra.clone(),
                                };
                                self.ready.push(new_proc);
                                return false;
                            }
                        }
                        true
                    });
                    if let Some(mut running_process) = self.running_process.take() {
                        running_process.state = ProcessState::Waiting { event: (Some(e)) };
                        running_process.timings.0 += usize::from(self.timeslice) - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += usize::from(self.timeslice) - remaining;
                        self.increase_timings(usize::from(self.timeslice) - remaining);
                        self.remaining_running_time = remaining;
                    }
                    SyscallResult::Success
                }
                Syscall::Exit => {
                    // fa update la timings si syscall
                    if let Some(running_process) = self.running_process.take() {
                        if running_process.pid == 1 {
                            self.init = true;
                        }
                    }
                    // increase all timings
                    self.increase_timings(usize::from(self.timeslice) - remaining);
                    self.running_process = None;
                    SyscallResult::Success
                }
            },
            crate::StopReason::Expired => {
                self.increase_timings(usize::from(self.timeslice));
                if let Some(mut running_process) = self.running_process.take() {
                    // Change its state and update the timings
                    running_process.state = ProcessState::Ready;
                    running_process.timings.0 += usize::from(self.timeslice);
                    running_process.timings.2 += usize::from(self.timeslice);
                    // Push to the ready queue
                    self.ready.push(running_process);
                }
                self.running_process = None;
                self.remaining_running_time = 0;
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
