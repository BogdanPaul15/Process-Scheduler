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
    ready: Vec<ProcessInfo>,              // ready queue
    wait: Vec<ProcessInfo>,               // wait queue
    pid_counter: usize,                   // used to increase pids
    running_process: Option<ProcessInfo>, // the currently running process
    remaining_running_time: usize,        // remaining running time
    init: bool,                           // to check if process with pid 1 exited
    sleep_amounts: Vec<usize>,            // keep track of sleeps amounts
    sleep: usize,                         // increase the timings when a process wakes up from sleep
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
        // Update timings for all processes and sleep amounts
        for proc in &mut self.ready {
            proc.timings.0 += amount;
        }
        for proc in &mut self.wait {
            proc.timings.0 += amount;
        }
        for sleep in &mut self.sleep_amounts {
            // An usize can't be negative
            if *sleep < amount {
                *sleep = 0;
            } else {
                *sleep -= amount;
            }
        }
        // Take the awakened processes from the queue and make them ready
        let mut zero_amount_indices = Vec::new();
        let mut proc_amount_indices = Vec::new();
        // Save the indices of the processes that have 0 amount to sleep
        for (index, &amount) in self.sleep_amounts.iter().enumerate() {
            if amount == 0 {
                zero_amount_indices.push(index);
            }
        }
        // Save the indexes of all sleeping processes from wait queue
        for (wait_index, proc) in self.wait.iter().enumerate() {
            if let ProcessState::Waiting { event } = &proc.state {
                if Option::is_none(event) {
                    proc_amount_indices.push(wait_index);
                }
            }
        }

        // Remove the sleep(0) processes, and then update the new indexes
        // (if you remove an element from a vec, the other indexes will be swapped with the index of the for loop)
        for (iter, i) in zero_amount_indices.iter().enumerate() {
            let new_index = i - iter;
            if let Some(index) = proc_amount_indices.get(new_index).cloned() {
                let mut proc = self.wait.remove(index);
                self.sleep_amounts.remove(new_index);
                proc.state = ProcessState::Ready;
                self.ready.push(proc);
            }
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
        // Increase all timings after a sleep (if 0, it will increase with 0)
        self.increase_timings(self.sleep);
        self.sleep = 0;

        match self.running_process.take() {
            Some(mut running_process) => {
                // If there is a running process, check if it can be rescheduled
                if self.remaining_running_time < self.minimum_remaining_timeslice {
                    // Can't reschedule, mark it as ready and push it to the ready queue
                    running_process.state = ProcessState::Ready;
                    self.ready.push(running_process);
                    // Get the first process from the ready queue and mark it as running
                    let mut proc = self.ready.remove(0);
                    proc.state = ProcessState::Running;
                    self.running_process = Some(proc);
                    self.remaining_running_time = self.timeslice.into();
                    // Return its pid and timeslice
                    return crate::SchedulingDecision::Run {
                        pid: self.running_process.as_ref().unwrap().pid(),
                        timeslice: NonZeroUsize::new(self.remaining_running_time).unwrap(),
                    };
                } else {
                    // Regain ownership
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
                    // Check for panic (if the process with pid 1 has exited)
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
                        // Wait queue is not empty, check for panic
                        if self.init {
                            self.init = false;
                            return crate::SchedulingDecision::Panic;
                        }
                        // Check for deadlock (there are only processes that wait for a signal in the wait queue)
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
                            // Sleep the processor for a minimum amount of time until some process wakes up
                            let mut min_amount = std::usize::MAX;
                            let mut min_index = 0;
                            // Compute the minimum and get its index
                            for (index, &amount) in self.sleep_amounts.iter().enumerate() {
                                if amount < min_amount {
                                    min_amount = amount;
                                    min_index = index;
                                }
                            }
                            // Remove its sleep amount
                            self.sleep_amounts.remove(min_index);
                            let mut wait_index = 0;
                            let mut target_wait_index = 0;

                            // Find it in the wait queue and remove it, then push it to the ready queue
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
                            // Save the minimum amount to update all timings in the next next
                            let proc = self.wait.remove(target_wait_index);
                            self.ready.push(proc);
                            self.sleep = min_amount;
                            return crate::SchedulingDecision::Sleep(
                                // Sleep the processor for a minimum amount of time
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
                        running_process.timings.2 += self.remaining_running_time - remaining - 1; // - 1 (the syscall)
                                                                                                  // Save the remaining time for the running process and regain ownership
                        self.remaining_running_time = remaining;
                        self.running_process = Some(running_process);
                    }
                    // Return the pid of the just created process
                    SyscallResult::Pid(new_pid)
                }
                Syscall::Sleep(amount) => {
                    // Increase all timings
                    self.increase_timings(self.remaining_running_time - remaining);
                    if let Some(mut running_process) = self.running_process.take() {
                        // Update the timings of the running process and push it to the wait queue
                        running_process.state = ProcessState::Waiting { event: None };
                        running_process.timings.0 += self.remaining_running_time - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += self.remaining_running_time - remaining - 1; // - 1 (the syscall)
                        self.wait.push(running_process);
                        // Push the sleep amount
                        self.sleep_amounts.push(amount);
                    }
                    // Reset the running process
                    self.remaining_running_time = self.timeslice.into();
                    self.running_process = None;
                    SyscallResult::Success
                }
                Syscall::Wait(e) => {
                    // Increase all timings
                    self.increase_timings(self.remaining_running_time - remaining);
                    if let Some(mut running_process) = self.running_process.take() {
                        // Update the timings of the running process and push it to the wait queue
                        running_process.state = ProcessState::Waiting { event: (Some(e)) };
                        running_process.timings.0 += self.remaining_running_time - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += self.remaining_running_time - remaining - 1; // - 1 (the syscall)
                        self.wait.push(running_process);
                    }
                    // Reset the running process
                    self.remaining_running_time = self.timeslice.into();
                    self.running_process = None;
                    SyscallResult::Success
                }
                Syscall::Signal(e) => {
                    // Increase all timings
                    self.increase_timings(self.remaining_running_time - remaining);
                    // Awaken all the processes that wait for the 'e' event
                    // First, save their indexes
                    let mut procs_to_ready = Vec::new();
                    for (index, proc) in self.wait.iter().enumerate() {
                        if let ProcessState::Waiting { event } = &proc.state {
                            if *event == Some(e) {
                                procs_to_ready.push(index);
                            }
                        }
                    }
                    // Remove them from the wait queue, mark them as Ready and push to the ready queue
                    for (index, i) in procs_to_ready.iter().enumerate() {
                        // Keep track of the modified index when removing
                        let modified_index = i - index;
                        let mut new_proc = self.wait.remove(modified_index);
                        new_proc.state = ProcessState::Ready;
                        self.ready.push(new_proc);
                    }
                    if let Some(mut running_process) = self.running_process.take() {
                        // Update the timings of the running process and the remaining time
                        running_process.timings.0 += self.remaining_running_time - remaining;
                        running_process.timings.1 += 1;
                        running_process.timings.2 += self.remaining_running_time - remaining - 1; // - 1 (the syscall)
                        self.remaining_running_time = remaining;
                        self.running_process = Some(running_process);
                    }
                    SyscallResult::Success
                }
                Syscall::Exit => {
                    // Increase all timings
                    self.increase_timings(self.remaining_running_time - remaining);
                    // Verify if process with pid 1 has exited
                    if let Some(running_process) = self.running_process.take() {
                        if running_process.pid == 1 {
                            self.init = true;
                        }
                    }
                    // Reset running process
                    self.remaining_running_time = self.timeslice.into();
                    self.running_process = None;
                    SyscallResult::Success
                }
            },
            crate::StopReason::Expired => {
                // Increase all timings
                self.increase_timings(self.remaining_running_time);
                if let Some(mut running_process) = self.running_process.take() {
                    // Change its state and update the timings
                    running_process.state = ProcessState::Ready;
                    running_process.timings.0 += self.remaining_running_time;
                    running_process.timings.2 += self.remaining_running_time;
                    // Push to the ready queue
                    self.ready.push(running_process);
                }
                // Reset the running process
                self.running_process = None;
                self.remaining_running_time = self.timeslice.into();
                SyscallResult::Success
            }
        }
    }

    fn list(&mut self) -> Vec<&dyn Process> {
        // List all processes from my Scheduler
        let mut list: Vec<&dyn Process> = Vec::new();
        for i in &self.ready {
            // Add the processes from the ready queue
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
