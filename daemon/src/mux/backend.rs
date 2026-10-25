//! Which multiplexer a `herdr ...` command goes to: the native mux, or herdr itself. bhote needs no herdr: where herdr is not installed, the mux answers.
//! Rules, in this order: BHOTE_MUX=native / =herdr say it; without a herdr program the mux answers; a command from inside one of herdr's own panes
//! (HERDR_SOCKET_PATH is herdr's socket, not the mux's) goes to herdr; else the mux when its server runs, herdr when it does not.
use std::sync::OnceLock;

/// Is there a herdr program on the PATH (looked up once)?
pub fn real_herdr() -> Option<std::path::PathBuf> {
    static R: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    R.get_or_init(|| {
        let me = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok());
        for d in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
            let c = d.join("herdr");
            // (the `herdr` link of the mux is this program, not herdr)
            if c.is_file() && c.canonicalize().ok() != me { return Some(c); }
        }
        None
    }).clone()
}

pub fn mux_running() -> bool { std::os::unix::net::UnixStream::connect(super::proto::sock_path()).is_ok() }

pub fn native() -> bool {
    match std::env::var("BHOTE_MUX").as_deref() { Ok("native") => return true, Ok("herdr") => return false, _ => {} }
    if real_herdr().is_none() { return true; }
    if let Some(s) = std::env::var_os("HERDR_SOCKET_PATH").filter(|s| !s.is_empty()) { if std::path::PathBuf::from(s) != super::proto::sock_path() { return false; } }
    mux_running()
}

/// Hands the command to herdr itself (this process becomes it).
pub fn exec_real(args: &[String]) -> ! {
    use std::os::unix::process::CommandExt;
    let Some(h) = real_herdr() else { eprintln!("herdr: not installed (and BHOTE_MUX=herdr)"); std::process::exit(127) };
    let e = std::process::Command::new(h).args(args).exec();
    eprintln!("herdr: {e}"); std::process::exit(127)
}
