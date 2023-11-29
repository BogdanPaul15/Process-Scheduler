use crate::Scheduler;

pub struct Empty;

impl Scheduler for Empty {
    fn next(&mut self) -> crate::SchedulingDecision {
        unimplemented!()
    }

    fn stop(&mut self, _reason: crate::StopReason) -> crate::SyscallResult {
        unimplemented!()
    }

    fn list(&mut self) -> Vec<&dyn crate::Process> {
        unimplemented!()
    }
}
