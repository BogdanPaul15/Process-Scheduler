use std::num::NonZeroUsize;

use crate::{Pid, Process, ProcessState, Scheduler, Syscall, SyscallResult};

pub struct ProcessInfo {
    pid: Pid,
    state: ProcessState,
    timings: (usize, usize, usize),
    priority: i8,
    _extra: String,
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
    sleep: usize,
}
impl RoundRobin {
    pub fn new(timeslice: NonZeroUsize, minimum_remaining_timeslice: usize) -> Self {
        Self {
            timeslice,
            minimum_remaining_timeslice,
            ready: Vec::new(),
            wait: Vec::new(),
            pid_counter: 1,
            running_process: None,
            remaining_running_time: timeslice.into(),
            init: false,
            sleep_amounts: Vec::new(),
            sleep: 0,
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
        self.increase_timings(self.sleep);
        self.sleep = 0;
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
                        crate::SchedulingDecision::Deadlock
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
                    if self.init {
                        self.init = false;
                        return crate::SchedulingDecision::Panic;
                    }
                    // Check if the process with pid 1 has exited
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
                                if Option::is_none(event) {
                                    is_deadlock = false;
                                    break;
                                }
                            }
                        }
                        if is_deadlock {
                            return crate::SchedulingDecision::Deadlock;
                        } else {
                            let mut min_amount = std::usize::MAX;
                            let mut min_index = 0;
                            for (index, &amount) in self.sleep_amounts.iter().enumerate() {
                                if amount < min_amount {
                                    min_amount = amount;
                                    min_index = index;
                                }
                            }
                            for amount in &mut self.sleep_amounts {
                                // if *amount - min_amount < 0 {
                                //     *amount = 0;
                                // } else {
                                *amount -= min_amount;
                                // }
                            }
                            self.sleep_amounts.remove(min_index);
                            let mut wait_index = 0;
                            let mut target_wait_index = 0;

                            for (index, proc) in self.wait.iter().enumerate() {
                                if let ProcessState::Waiting { event } = &proc.state {
                                    if Option::is_none(event) {
                                        if wait_index == min_index {
                                            target_wait_index = index;
                                            break;
                                        }
                                        wait_index += 1;
                                    }
                                }
                            }
                            let proc = self.wait.remove(target_wait_index);
                            self.ready.push(proc);
                            self.sleep = min_amount;
                            return crate::SchedulingDecision::Sleep(
                                NonZeroUsize::new(min_amount).unwrap(),
                            );
                        }
                    }
                    // Handle the case when there's no process available to run
                    crate::SchedulingDecision::Done
                }
            }
        }
    }

    fn stop(&mut self, _reason: crate::StopReason) -> crate::SyscallResult {
        // Check the indices with zero
        let mut zero_amount_indices = Vec::new();
        let mut proc_amount_indices = Vec::new();
        for (index, &amount) in self.sleep_amounts.iter().enumerate() {
            if amount == 0 {
                zero_amount_indices.push(index);
            }
        }
        for (wait_index, proc) in self.wait.iter().enumerate() {
            if let ProcessState::Waiting { event } = &proc.state {
                if Option::is_none(event) {
                    proc_amount_indices.push(wait_index);
                }
            }
        }

        for i in zero_amount_indices {
            if let Some(index) = proc_amount_indices.get(i).cloned() {
                let mut proc = self.wait.remove(index);
                self.sleep_amounts.remove(i);
                proc.state = ProcessState::Ready;
                self.ready.push(proc);
            }
        }

        match _reason {
            crate::StopReason::Syscall { syscall, remaining } => match syscall {
                Syscall::Fork(priority) => {
                    // Increase all total timings
                    self.increase_timings(self.remaining_running_time - remaining);
                    // Generate a new process
                    let new_pid = self.generate_pid();
                    let new_process = ProcessInfo {
                        pid: new_pid,
                        state: ProcessState::Ready,
                        timings: (0, 0, 0),
                        priority,
                        _extra: String::new(),
                    };
                    // Add it to the ready queue
                    self.ready.push(new_process);
                    if let Some(mut running_process) = self.running_process.take() {
                        // Update the timings of the running process
                        running_process.timings.0 += self.remaining_running_time - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += self.remaining_running_time - remaining - 1;
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
                        running_process.timings.0 += self.remaining_running_time - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += self.remaining_running_time - remaining - 1;
                        self.increase_timings(self.remaining_running_time - remaining);
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
                        running_process.timings.0 += self.remaining_running_time - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += self.remaining_running_time - remaining - 1;
                        self.increase_timings(self.remaining_running_time - remaining);
                        self.wait.push(running_process);
                    }
                    self.running_process = None;
                    SyscallResult::Success
                }
                Syscall::Signal(e) => {
                    let mut index = 0;
                    let mut procs_to_ready = Vec::new();
                    for proc in &self.wait {
                        if let ProcessState::Waiting { event } = &proc.state {
                            if *event == Some(e) {
                                procs_to_ready.push(index);
                            }
                        }
                        index += 0;
                    }
                    for i in procs_to_ready {
                        let mut new_proc = self.wait.remove(i);
                        new_proc.state = ProcessState::Ready;
                        self.ready.push(new_proc);
                    }
                    if let Some(mut running_process) = self.running_process.take() {
                        running_process.state = ProcessState::Waiting { event: (Some(e)) };
                        running_process.timings.0 += usize::from(self.timeslice) - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += usize::from(self.timeslice) - remaining;
                        self.increase_timings(usize::from(self.timeslice) - remaining);
                        self.remaining_running_time = remaining;
                        self.running_process = Some(running_process);
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
                    self.increase_timings(self.remaining_running_time - remaining);
                    self.running_process = None;
                    SyscallResult::Success
                }
            },
            crate::StopReason::Expired => {
                self.increase_timings(self.remaining_running_time);
                if let Some(mut running_process) = self.running_process.take() {
                    // Change its state and update the timings
                    running_process.state = ProcessState::Ready;
                    running_process.timings.0 += self.remaining_running_time;
                    running_process.timings.2 += self.remaining_running_time;
                    // Push to the ready queue
                    self.ready.push(running_process);
                }
                self.running_process = None;
                self.remaining_running_time = self.timeslice.into();
                SyscallResult::Success
            }
        }
    }

    fn list(&mut self) -> Vec<&dyn Process> {
        let mut list: Vec<&dyn Process> = Vec::new();
        for i in &self.ready {
            list.push(i)
        }
        for i in &self.wait {
            list.push(i)
        }
        if let Some(x) = &self.running_process {
            list.push(x);
        }
        list
    }
}
