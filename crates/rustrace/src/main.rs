use anyhow::{anyhow, bail, Result};
use clap::Parser;
use fork::{fork, Fork};
use nix::{
    errno::Errno,
    sys::{
        ptrace::{self, traceme, Options},
        wait::waitpid,
    },
    unistd::Pid,
};
use syscalls::Sysno;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Command to trace
    cmd: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    match fork() {
        Ok(Fork::Parent(child)) => trace_child(child),
        Ok(Fork::Child) => run_command_to_trace(&args.cmd),
        Err(e) => Err(anyhow!("{e}")),
    }
}

fn trace_child(child: i32) -> Result<()> {
    let child = Pid::from_raw(child);
    waitpid(Some(child), None)?;
    // Jail the child
    ptrace::setoptions(child, Options::PTRACE_O_EXITKILL)?;

    loop {
        // Stop on syscall invocation
        ptrace::syscall(child, None)?;
        waitpid(Some(child), None)?;

        let registers = ptrace::getregs(child)?;
        let syscall: Sysno = (registers.orig_rax as i32).into();
        print!(
            "{syscall}({}, {}, {}, {}, {}, {})",
            registers.rdi, registers.rsi, registers.rdx, registers.r10, registers.r8, registers.r9
        );

        // Run the syscall
        ptrace::syscall(child, None)?;
        waitpid(Some(child), None)?;
        match ptrace::getregs(child) {
            Ok(registers) => println!(" = {}", registers.rax),
            Err(e) => {
                println!(" = ?");
                if e == Errno::ESRCH {
                    bail!("Child exited, terminating trace.");
                }
            }
        };
    }
}

fn run_command_to_trace(cmd: &str) -> Result<()> {
    let command_arguments = cmd.split_whitespace().collect::<Vec<_>>();
    traceme()?;
    let err = exec::execvp(command_arguments[0], &command_arguments);
    return Err(anyhow!("{err}"));
}
