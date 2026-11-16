//! Giving freed memory back to the system.

/// glibc keeps what a thread frees in that thread's arena and on its own returns
/// only the top of each heap. After a 20,000-entry folder was left for a small
/// one, about 12 MB stayed with the process, all of it free. Call off the UI
/// thread: it locks each arena in turn.
pub fn release_freed() {
    let _span = crate::profile::span("release freed memory");
    // SAFETY: malloc_trim only hands free pages back to the system.
    unsafe { libc::malloc_trim(0) };
}
