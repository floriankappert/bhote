//! What is running: the command line and the working folder of a process, without starting `ps` or `lsof` (Linux: /proc; macOS: sysctl and libproc).
//! Both are system calls that take microseconds, so they can be asked on the event loop of the mux.

#[cfg(target_os = "linux")]
pub fn cmdline(pid: i32) -> Option<String> {
    let b = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let s = String::from_utf8_lossy(&b).replace('\0', " ").trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(target_os = "linux")]
pub fn cwd(pid: i32) -> Option<String> { std::fs::read_link(format!("/proc/{pid}/cwd")).ok().map(|p| p.to_string_lossy().into_owned()) }

/// The argument vector of a process as macOS keeps it: argc, the program's path, padding, then argc strings (what follows is the environment).
#[cfg(target_os = "macos")]
pub fn cmdline(pid: i32) -> Option<String> {
    let mut size: libc::size_t = 0;
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    unsafe {
        if libc::sysctl(mib.as_mut_ptr(), 3, std::ptr::null_mut(), &mut size, std::ptr::null_mut(), 0) != 0 || size < 8 { return None; }
        let mut buf = vec![0u8; size];
        if libc::sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr() as *mut libc::c_void, &mut size, std::ptr::null_mut(), 0) != 0 { return None; }
        buf.truncate(size);
        parse_procargs2(&buf)
    }
}

/// (also used by the test below, on any system)
#[allow(dead_code)]
pub fn parse_procargs2(buf: &[u8]) -> Option<String> {
    if buf.len() < 5 { return None; }
    let argc = i32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]).max(0) as usize;
    let mut i = 4;
    while i < buf.len() && buf[i] != 0 { i += 1; }            // the executable's path
    while i < buf.len() && buf[i] == 0 { i += 1; }            // its padding
    let mut args: Vec<String> = Vec::new();
    while args.len() < argc && i < buf.len() {
        let s = i; while i < buf.len() && buf[i] != 0 { i += 1; }
        args.push(String::from_utf8_lossy(&buf[s..i]).into_owned());
        i += 1;
    }
    if args.is_empty() { None } else { Some(args.join(" ")) }
}

#[cfg(target_os = "macos")]
pub fn cwd(pid: i32) -> Option<String> {
    unsafe {
        let mut info: libc::proc_vnodepathinfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
        let n = libc::proc_pidinfo(pid, libc::PROC_PIDVNODEPATHINFO, 0, &mut info as *mut _ as *mut libc::c_void, size);
        if n < size { return None; }
        let p = info.pvi_cdir.vip_path.as_ptr() as *const u8;
        let bytes = std::slice::from_raw_parts(p, std::mem::size_of_val(&info.pvi_cdir.vip_path));
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        if end == 0 { None } else { Some(String::from_utf8_lossy(&bytes[..end]).into_owned()) }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn cmdline(_pid: i32) -> Option<String> { None }
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn cwd(_pid: i32) -> Option<String> { None }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn procargs2_layout() {
        let mut b = 3i32.to_ne_bytes().to_vec();
        b.extend_from_slice(b"/usr/bin/claude\0\0\0\0claude\0--resume\0abc\0HOME=/x\0");
        assert_eq!(parse_procargs2(&b).as_deref(), Some("claude --resume abc"));
        assert_eq!(parse_procargs2(&[0, 0]), None);
    }
    #[test]
    fn my_own_process() {
        let me = std::process::id() as i32;
        assert!(cmdline(me).is_some());
        assert_eq!(cwd(me).map(std::path::PathBuf::from), std::env::current_dir().ok().and_then(|d| d.canonicalize().ok()));
    }
}
