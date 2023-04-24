use clap::Parser;
use composer::api::{Client, Event};
use eyre::Result;
use nix::{
    sys::{ptrace, wait::waitpid},
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

    let client = args
        .address
        .map(Client::new)
        .unwrap_or(Client::try_default())?;

    ptrace::attach(pid)?;
    waitpid(Some(pid), None)?;
    loop {
        let Some(event) = handle_syscall(pid)? else { continue; };
        if let Err(err) = client.send(&event) {
            eprintln!("Could not send event {:?}", err)
        };
    }
}

fn handle_syscall(pid: Pid) -> Result<Option<Event>, color_eyre::Report> {
    ptrace::syscall(pid, None)?;
    waitpid(Some(pid), None)?;
    let registers = ptrace::getregs(pid)?;
    let syscall: Sysno = (registers.orig_rax as i32).into();
    let rdi = registers.rdi;

    // At this point, the syscall is suspended, we need to call `syscall`
    // again to actually execute it and retrieve the return value from `rax`.
    ptrace::syscall(pid, None)?;
    waitpid(Some(pid), None)?;

    match (syscall, rdi) {
        (Sysno::write, 1) => Ok(Some(Event::StdoutWrite {
            length: ptrace::getregs(pid)?.rax as usize,
        })),
        (Sysno::write, 2) => Ok(Some(Event::StderrWrite {
            length: ptrace::getregs(pid)?.rax as usize,
        })),
        _ => Ok(None),
    }
}
