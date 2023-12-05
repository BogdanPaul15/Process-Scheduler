[![Review Assignment Due Date](https://classroom.github.com/assets/deadline-readme-button-24ddc0f5d75046c5622901739e7c5dd533143b0c8e959d652212380cedb1ea36.svg)](https://classroom.github.com/a/2eN9hsMw)

# Process Scheduler

## Implementation

I've only used unwrap() in cases that panic is not possible, like:
- when returning the pid of the currently running process (unwrap() is used because pid contains a NonZeroUsize): the pid can't be 0 because my **pid_counter** is initialized with 1 and increased by 1 everytime. So it can't be 0;
- when returning the timeslice, **remaining_running_time** can't be 0 because it is initialized with the timeslice and always when it needs to be reset, it receives the value of timeslice; 
- **min_amount** of sleep can't be 0 because until that point all amounts of zero are removed from the sleep_amounts vec and processes are pushed to the ready queue, so no 0 amount is possible when computing the min_amount of sleep.

#### **Round Robin**

##### Details

For the Round Robin algorithm, I've used two structures to keep my process data. One is `ProcessInfo` which holds the data of a process and has the basic fields: **pid**, **state**, **timings**, **priority**, and **extra**. For this algorithm, I completely ignored the expired and priority fields. The other structure is named `RoundRobin`, which has the following fields: two queues (**ready**, **wait**) to easily store processes, **timeslice**, and **minimum_remaining_timeslice**, **pid_counter** (this is used to generate a new pid every time a new process is created via *fork*), **running_process** (an *Option<ProcessInfo>* which keeps track of the currently running process), **init** which is in case of process with pid 1 exited, **sleep_amounts** (update and keeps track of the sleep amount for the sleep() processes from the wait queue), **sleep** (used to keep track of the total time the processor has slept).

`RoundRobin` structure has its own implementation, a constructor **new()** which initializes the structure when it is called in *lib.rs*, a method that generates a new pid based on the **pid_counter** field, a **increase_timings** method which increases all total timing for all processes in the ready and wait queue, decreases all amounts of sleep in the **sleep_amounts** and checks if there are sleep processes in the wait queue that have woken up to mark them as ready and move to the ready queue.

##### **next()**

Firstly, I increase all timings with the amount that the processor has slept (if the processor did not sleep, this will be 0). Then I verify if there is a currently running process on the processor. If there is a currently running process, I check if it can be rescheduled or not (if the remaining run time is smaller than minimum remaining timeslice). If yes, the currently running process can't be rescheduled, I change its state into Ready and push it to the ready queue. Then I get the first ready process from the ready queue and mark it as the currently running process and return its pid and timeslice. If the currently running process can be rescheduled, I return its pid and timeslice.

If there is no currently running process on the processor, If the ready queue is not empty, I check for panic (process with pid 1 has exited) and then return the first process from the ready queue. If the ready queue is empty and the wait queue is not, I also check for panic and then I check if there is a deadlock (iterate over all the processes from the wait queue and check if there is at least one sleeping process). If no deadlock occurs, the processor has to sleep for the minimum amount until a process wakes up from sleep because it has no process to schedule next. So, in the sleep_amount, I find the minimum sleep amount and save its index in the sleep_amounts. Then I iterate over the wait queue and get the index of the process with min sleep amount to remove it from the wait queue and push it to the ready. This is where I save the sleep amount to update all the timings in the next next. If we are not in any of the options above, then return Done (no more processes available for schedule).

##### **stop()**

Based on the reason for the stop, I used a match to go through all possible cases like so:
- Expired -> the currently running process has expired, so I increase all the timings with the remaining running time, I update the timings of the running process also and change its state to ready, push it to the ready queue, and reset the currently running process.
- Syscall:
  - Fork - increase all timings, generate a new process, add it to the ready queue, also increase timings for the running process, update the remaining time (so in the next, we can decide if it is rescheduled or not), and return the pid of the just created process;
  - Sleep - increase all timings, change the state of the currently running process to waiting for event: none, update its timings, push it to the wait queue, push the sleep amount also, and reset the currently running process;
  - Wait - increase all timings, change the state to waiting for the given event, push it to the wait queue, and reset the currently running process;
  - Signal - increase all timings, iterate over the wait queue and find all the indexes of processes that are waiting for this signal event. Then remove them all in order from the wait queue (indexes are decreasing when removing, so the new index to remove is the process index - currently index in the iteration), mark them as Ready, and push them to the ready queue. Update the timings of the currently running process, the remaining running time, and regain ownership over the running process;
  - Exit - increase all timings, if the currently running process that just exited has pid 1, mark it in the init field and reset the currently running process.

##### **list()**

Adds to a **Vec<&dyn Process>** all the processes that are in the system (from the ready queue, the wait queue, and also the currently running process) and returns it.

#### **Round Robin with priority**

##### Details

Almost the same fields as Round Robin, but `ProcessInfo` also receives a **default_priority** field where it keeps the priority received at the time of the creation.

Same implementation as round robin, only that when the current process receives a syscall, the priority is increased by 1, and when it expires, it is decreased by 1, keeping the imposed limits (the priority cannot be lower than 0 or higher than the default priority).

Then, whenever a process is added to the ready queue, the ready queue is sorted in descending order by priority so that the processes with higher priority are first.


## Getting started

Please run `cargo doc --open` to create and open the documentation.

Your job is:
1. Implement the schedulers in the `scheduler` crate in the folder `scheduler/src/schedulers`.
2. Export the scheduler in the `scheduler/src/lib.rs` file using the three functions
   - `round_robin(...)`
   - `priority_queue(...)`
   - `cfs(...)`
3. Test them using the `runner` crate by using them in `runner/src/main.rs`.
