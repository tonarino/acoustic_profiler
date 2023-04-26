use dtrace_sys as sys;
use std::{
    ffi::{CStr, CString},
    fmt::{self, Display, Formatter},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to initialize a DTrace instance.
    InitializationError(String),
    /// Failed to compile a DTrace program.
    ProgramCompilationError(String),
    /// An invalid option or value requested.
    InvalidOption(String),
    /// Failed to execute a DTrace operation.
    OperationError(String),
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        write!(fmt, "{self:?}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramStatus {
    /// There is an ongoing program execution.
    Ongoing,
    /// The program has completed.
    Done,
}

#[derive(Debug)]
pub struct ProbeData {
    /// ID of the CPU where this probe was executed.
    pub cpu_id: i32,
    /// Probe provider name.
    pub provider_name: String,
    /// Probe module name.
    pub module_name: String,
    /// Probe function name.
    pub function_name: String,
    /// Probe name.
    pub name: String,
    // TODO(skywhale): Add other data such as function arguments.
}

/// The result of `wait_and_consume()` function.
#[derive(Debug)]
pub struct WaitAndConsumeResult {
    /// The status of the program that is being executed.
    pub status: ProgramStatus,
    /// Probe data that was collected during the wait.
    pub probes: Vec<ProbeData>,
}

fn c_char_to_string(raw: *const i8) -> String {
    unsafe { CStr::from_ptr(raw) }.to_string_lossy().to_string()
}

impl ProbeData {
    fn new(raw: *const sys::dtrace_probedata_t) -> Self {
        let cpu_id = unsafe { (*raw).dtpda_cpu };
        let desc = unsafe { *(*raw).dtpda_pdesc };
        let provider_name = c_char_to_string(desc.dtpd_provider.as_ptr());
        let module_name = c_char_to_string(desc.dtpd_mod.as_ptr());
        let name = c_char_to_string(desc.dtpd_name.as_ptr());
        let function_name = c_char_to_string(desc.dtpd_func.as_ptr());
        Self {
            cpu_id,
            provider_name,
            module_name,
            function_name,
            name,
        }
    }
}

extern "C" fn probe_callback(
    probe_data_raw: *const sys::dtrace_probedata_t,
    user_ptr: *mut std::ffi::c_void,
) -> i32 {
    let probe_data = ProbeData::new(probe_data_raw);

    let dtrace: &mut DTrace = unsafe { &mut *(user_ptr as *mut DTrace) };
    dtrace.probes.push(probe_data);

    sys::DTRACE_CONSUME_THIS as i32
}

/// Taken from:
/// https://docs.oracle.com/cd/E88353_01/html/E37842/libdtrace-3lib.html
extern "C" fn probe_action_callback(
    _probe_data_raw: *const sys::dtrace_probedata_t,
    action_data_raw: *const sys::dtrace_recdesc_t,
    _user_ptr: *mut std::ffi::c_void,
) -> i32 {
    if action_data_raw.is_null()
        || unsafe { *action_data_raw }.dtrd_action as u32 == sys::DTRACEACT_EXIT
    {
        sys::DTRACE_CONSUME_NEXT as i32
    } else {
        sys::DTRACE_CONSUME_THIS as i32
    }
}

// TODO(skywhale): Support grabbing a process for user-space inspection.
#[derive(Debug)]
pub struct DTrace {
    inner: *mut sys::dtrace_hdl_t,
    probes: Vec<ProbeData>,
}

impl DTrace {
    pub fn new() -> Result<Self, Error> {
        let flags = 0i32;
        let mut code = 0i32;
        let dtrace = unsafe { sys::dtrace_open(sys::DTRACE_VERSION as i32, flags, &mut code) };
        if dtrace.is_null() {
            let message_raw = unsafe { sys::dtrace_errmsg(std::ptr::null_mut(), code) };
            return Err(Error::InitializationError(c_char_to_string(message_raw)));
        }

        Ok(Self {
            inner: dtrace,
            probes: Vec::default(),
        })
    }

    /// Compiles a D program and starts collecting probe data.
    ///
    /// Available DTrace options:
    /// https://docs.oracle.com/en/operating-systems/solaris/oracle-solaris/11.4/dtrace-guide/consumer-options.html
    // TODO(skywhale): Define DtraceOption to get type safety.
    pub fn execute_program(&self, program: &str, options: &[(&str, &str)]) -> Result<(), Error> {
        let program = CString::new(program).expect("CString::new failed");
        let prog = unsafe {
            sys::dtrace_program_strcompile(
                self.inner,
                program.as_ptr(),
                sys::dtrace_probespec_DTRACE_PROBESPEC_NAME,
                0,
                0,
                std::ptr::null(),
            )
        };
        if prog.is_null() {
            return Err(Error::ProgramCompilationError(self.last_error_message()));
        }

        let mut info = sys::dtrace_proginfo_t::default();
        if unsafe { sys::dtrace_program_exec(self.inner, prog, &mut info) } == -1 {
            return Err(Error::OperationError(self.last_error_message()));
        }

        for option in options.iter() {
            let name = CString::new(option.0).expect("CString::new failed");
            let value = CString::new(option.1).expect("CString::new failed");
            if unsafe { sys::dtrace_setopt(self.inner, name.as_ptr(), value.as_ptr()) } == -1 {
                return Err(Error::InvalidOption(self.last_error_message()));
            }
        }

        if unsafe { sys::dtrace_go(self.inner) } != 0 {
            return Err(Error::OperationError(self.last_error_message()));
        }

        Ok(())
    }

    /// Blocks for some time and consumes the probe data collected by DTrace. The |ProgramStatus|
    /// informs about the state of the program execution.
    ///
    /// The duration it blocks depends on `switchrate`, `statusrate` and `aggrate` options used for
    /// the program execution.
    pub fn wait_and_consume(
        &mut self,
    ) -> Result<WaitAndConsumeResult, Error> {
        unsafe {
            sys::dtrace_sleep(self.inner);
        }

        let user_ptr = &mut *self as *mut _ as *mut std::ffi::c_void;
        let status_raw = unsafe {
            sys::dtrace_work(
                self.inner,
                std::ptr::null_mut(),
                Some(probe_callback),
                Some(probe_action_callback),
                user_ptr,
            )
        };

        let status = match status_raw {
            sys::dtrace_workstatus_t_DTRACE_WORKSTATUS_OKAY => ProgramStatus::Ongoing,
            sys::dtrace_workstatus_t_DTRACE_WORKSTATUS_DONE => ProgramStatus::Done,
            _ => return Err(Error::OperationError(self.last_error_message())),
        };

        let probes = std::mem::take(&mut self.probes);

        Ok(WaitAndConsumeResult {
            status,
            probes,
        })
    }

    /// Instructs the kernel to disable any enabled probe and free the memory.
    pub fn stop(&self) -> Result<(), Error> {
        if unsafe { sys::dtrace_stop(self.inner) } == -1 {
            return Err(Error::OperationError(self.last_error_message()));
        }
        Ok(())
    }

    fn last_error_message(&self) -> String {
        let message_raw = unsafe { sys::dtrace_errmsg(self.inner, sys::dtrace_errno(self.inner)) };
        c_char_to_string(message_raw)
    }
}

impl Drop for DTrace {
    fn drop(&mut self) {
        unsafe {
            sys::dtrace_close(self.inner);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{DTrace, Error, ProgramStatus};

    #[test]
    fn dtrace_open_close() -> Result<(), Error> {
        let mut dtrace = DTrace::new()?;

        dtrace.execute_program(&format!("syscall:::entry {{}}"), &[("bufsize", "1k")])?;

        let result = dtrace.wait_and_consume()?;
        assert_eq!(
            ProgramStatus::Ongoing,
            result.status,
        );
        assert!(result.probes.len() > 0);

        dtrace.stop()?;

        Ok(())
    }
}
