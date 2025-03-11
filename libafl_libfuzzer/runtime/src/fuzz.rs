use core::ffi::c_int;
#[cfg(unix)]
use std::io::{Write, stderr, stdout};
use std::{fmt::Debug, fs::File, net::TcpListener, os::fd::AsRawFd, str::FromStr};
use libafl::Error;
use libafl_bolts::shmem::StdShMemProvider;

fn do_fuzz<F, ST, E, I, S, EM>(
    fuzzer: &mut F,
    stages: &mut ST,
    executor: &mut E,
    state: &mut S,
    mgr: &mut EM,
) -> Result<(), Error>
{
    Ok(())
}

fn fuzz_single_forking<M>(
    harness: &extern "C" fn(*const u8, usize) -> c_int,
    mut shmem_provider: StdShMemProvider,
    monitor: M,
) -> Result<(), Error>
{
    Ok(())
}

/// Communicate the selected port to subprocesses
const PORT_PROVIDER_VAR: &str = "_LIBAFL_LIBFUZZER_FORK_PORT";

fn fuzz_many_forking<M>(
    harness: &extern "C" fn(*const u8, usize) -> c_int,
    shmem_provider: StdShMemProvider,
    forks: usize,
    monitor: M,
) -> Result<(), Error>
{
    Ok(())
}

pub fn fuzz(
    harness: &extern "C" fn(*const u8, usize) -> c_int,
) -> Result<(), Error> {
    Ok(())
}
