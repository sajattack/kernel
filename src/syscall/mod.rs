///! Syscall handlers

extern crate syscall;

pub use self::syscall::{data, error, flag, io, number, scheme};

pub use self::driver::*;
pub use self::fs::*;
pub use self::futex::futex;
pub use self::privilege::*;
pub use self::process::*;
pub use self::time::*;
pub use self::validate::*;

use self::data::{SigAction, TimeSpec};
use self::error::{Error, Result, ENOSYS};
use self::number::*;

use context::ContextId;
use interrupt::syscall::SyscallStack;
use scheme::{FileHandle, SchemeNamespace};

/// Debug
pub mod debug;

/// Driver syscalls
pub mod driver;

/// Filesystem syscalls
pub mod fs;

/// Fast userspace mutex
pub mod futex;

/// Privilege syscalls
pub mod privilege;

/// Process syscalls
pub mod process;

/// Time syscalls
pub mod time;

/// Validate input
pub mod validate;

//mod print_call;
//use self::print_call::print_call;


#[no_mangle]
pub extern fn syscall(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize, bp: usize, stack: &mut SyscallStack) -> usize {
    #[inline(always)]
    fn inner(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize, bp: usize, stack: &mut SyscallStack) -> Result<usize> {
        match a & SYS_CLASS {
            SYS_CLASS_FILE => {
                let fd = FileHandle::from(b);
                match a & SYS_ARG {
                    SYS_ARG_SLICE => file_op_slice(a, fd, validate_slice(c as *const u8, d)?),
                    SYS_ARG_MSLICE => file_op_mut_slice(a, fd, validate_slice_mut(c as *mut u8, d)?),
                    _ => match a {
                        SYS_CLOSE => close(fd),
                        SYS_DUP => dup(fd, validate_slice(c as *const u8, d)?).map(FileHandle::into),
                        SYS_DUP2 => dup2(fd, FileHandle::from(c), validate_slice(d as *const u8, e)?).map(FileHandle::into),
                        SYS_FCNTL => fcntl(fd, c, d),
                        SYS_FEVENT => fevent(fd, c),
                        SYS_FUNMAP => funmap(b),
                        _ => file_op(a, fd, c, d)
                    }
                }
            },
            SYS_CLASS_PATH => match a {
                SYS_OPEN => open(validate_slice(b as *const u8, c)?, d).map(FileHandle::into),
                SYS_CHMOD => chmod(validate_slice(b as *const u8, c)?, d as u16),
                SYS_RMDIR => rmdir(validate_slice(b as *const u8, c)?),
                SYS_UNLINK => unlink(validate_slice(b as *const u8, c)?),
                _ => Err(Error::new(ENOSYS))
            },
            _ => match a {
                SYS_YIELD => sched_yield(),
                SYS_NANOSLEEP => nanosleep(
                    validate_slice(b as *const TimeSpec, 1).map(|req| &req[0])?,
                    if c == 0 {
                        None
                    } else {
                        Some(validate_slice_mut(c as *mut TimeSpec, 1).map(|rem| &mut rem[0])?)
                    }
                ),
                SYS_CLOCK_GETTIME => clock_gettime(b, validate_slice_mut(c as *mut TimeSpec, 1).map(|time| &mut time[0])?),
                SYS_FUTEX => futex(validate_slice_mut(b as *mut i32, 1).map(|uaddr| &mut uaddr[0])?, c, d as i32, e, f as *mut i32),
                SYS_BRK => brk(b),
                SYS_GETPID => getpid().map(ContextId::into),
                SYS_GETPGID => getpgid(ContextId::from(b)).map(ContextId::into),
                SYS_GETPPID => getppid().map(ContextId::into),
                SYS_CLONE => clone(b, bp).map(ContextId::into),
                SYS_EXIT => exit((b & 0xFF) << 8),
                SYS_KILL => kill(ContextId::from(b), c),
                SYS_WAITPID => waitpid(ContextId::from(b), c, d).map(ContextId::into),
                SYS_CHDIR => chdir(validate_slice(b as *const u8, c)?),
                SYS_EXECVE => exec(validate_slice(b as *const u8, c)?, validate_slice(d as *const [usize; 2], e)?),
                SYS_IOPL => iopl(b, stack),
                SYS_GETCWD => getcwd(validate_slice_mut(b as *mut u8, c)?),
                SYS_GETEGID => getegid(),
                SYS_GETENS => getens(),
                SYS_GETEUID => geteuid(),
                SYS_GETGID => getgid(),
                SYS_GETNS => getns(),
                SYS_GETUID => getuid(),
                SYS_MKNS => mkns(validate_slice(b as *const [usize; 2], c)?),
                SYS_SETPGID => setpgid(ContextId::from(b), ContextId::from(c)),
                SYS_SETREUID => setreuid(b as u32, c as u32),
                SYS_SETRENS => setrens(SchemeNamespace::from(b), SchemeNamespace::from(c)),
                SYS_SETREGID => setregid(b as u32, c as u32),
                SYS_SIGACTION => sigaction(
                    b,
                    if c == 0 {
                        None
                    } else {
                        Some(validate_slice(c as *const SigAction, 1).map(|act| &act[0])?)
                    },
                    if d == 0 {
                        None
                    } else {
                        Some(validate_slice_mut(d as *mut SigAction, 1).map(|oldact| &mut oldact[0])?)
                    },
                    e
                ),
                SYS_SIGRETURN => sigreturn(),
                SYS_PIPE2 => pipe2(validate_slice_mut(b as *mut usize, 2)?, c),
                SYS_PHYSALLOC => physalloc(b),
                SYS_PHYSFREE => physfree(b, c),
                SYS_PHYSMAP => physmap(b, c, d),
                SYS_PHYSUNMAP => physunmap(b),
                SYS_VIRTTOPHYS => virttophys(b),
                _ => Err(Error::new(ENOSYS))
            }
        }
    }

    /*
    let debug = {
        let contexts = ::context::contexts();
        if let Some(context_lock) = contexts.current() {
            let context = context_lock.read();
            if unsafe { ::core::str::from_utf8_unchecked(&context.name.lock()) } == "file:/bin/acid" {
                if (a == SYS_WRITE || a == SYS_FSYNC) && (b == 1 || b == 2) {
                    false
                } else {
                    true
                }
            } else {
                false
            }
        } else {
            false
        }
    };

    if debug {
        let contexts = ::context::contexts();
        if let Some(context_lock) = contexts.current() {
            let context = context_lock.read();
            print!("{} ({}): ", unsafe { ::core::str::from_utf8_unchecked(&context.name.lock()) }, context.id.into());
        }

        let _ = debug::print_call(a, b, c, d, e, f);
        println!("");
    }
    */

    {
        let contexts = ::context::contexts();
        if let Some(context_lock) = contexts.current() {
            let mut context = context_lock.write();
            context.syscall = Some((a, b, c, d, e, f));
        }
    }

    let result = inner(a, b, c, d, e, f, bp, stack);

    {
        let contexts = ::context::contexts();
        if let Some(context_lock) = contexts.current() {
            let mut context = context_lock.write();
            context.syscall = None;
        }
    }

    /*
    if debug {
        let contexts = ::context::contexts();
        if let Some(context_lock) = contexts.current() {
            let context = context_lock.read();
            print!("{} ({}): ", unsafe { ::core::str::from_utf8_unchecked(&context.name.lock()) }, context.id.into());
        }

        let _ = debug::print_call(a, b, c, d, e, f);

        match result {
            Ok(ref ok) => {
                println!(" = Ok({} ({:#X}))", ok, ok);
            },
            Err(ref err) => {
                println!(" = Err({} ({:#X}))", err, err.errno);
            }
        }
    }
    */

    Error::mux(result)
}
