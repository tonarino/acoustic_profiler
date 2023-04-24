use clap::Parser;
use composer::api::{Client, Event};
use eyre::{bail, Result};
use nix::{
    errno::Errno,
    sys::{
        ptrace::{self, Options},
        wait::waitpid,
    },
    unistd::Pid,
};
use syscalls::Sysno;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// PID to attach to and trace
    pid: u32,

    /// Server address to receive events
    address: Option<String>,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let pid = Pid::from_raw(args.pid as i32);

    ptrace::attach(pid)?;
    waitpid(Some(pid), None)?;
    loop {
        // Stop on syscall invocation
        ptrace::syscall(pid, None)?;
        waitpid(Some(pid), None)?;

        let registers = ptrace::getregs(pid)?;
        let syscall: Sysno = (registers.orig_rax as i32).into();
        print!(
            "{syscall}({}, {}, {}, {}, {}, {})",
            registers.rdi, registers.rsi, registers.rdx, registers.r10, registers.r8, registers.r9
        );

        // Run the syscall
        ptrace::syscall(pid, None)?;
        waitpid(Some(pid), None)?;
        match ptrace::getregs(pid) {
            Ok(registers) => println!(" = {}", registers.rax),
            Err(e) => {
                println!(" = ?");
                if e == Errno::ESRCH {
                    bail!("Probe target exited.");
                }
            }
        };
    }
}
