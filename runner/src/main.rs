use scheduler::round_robin;
use std::num::NonZeroUsize;

use processor::format_logs;
use processor::Processor;

fn main() {
    let logs = Processor::run(round_robin(NonZeroUsize::new(2).unwrap(), 1), |process| {
        process.exec();
        process.exec();
        process.exec();
        process.exec();
        process.fork(
            |process| {
                process.exec();
                process.exec();
                process.wait(1);
            },
            0,
        );
        process.sleep(10);
        process.signal(1);
        process.exec();
    });

    println!("{}", format_logs(&logs));
}

// Do not delete this line
#[cfg(test)]
mod tests;
