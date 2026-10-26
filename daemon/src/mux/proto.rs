//! Socket path and the frames of an attach connection (docs/mux-protocol.md).
use std::io::{self, Read, Write};
use std::path::PathBuf;

pub fn sock_path() -> PathBuf {
    if let Some(p) = std::env::var_os("BHOTE_MUX_SOCK") { return PathBuf::from(p); }
    // the same place for every process of the user: also where XDG_RUNTIME_DIR is not set (a non-login ssh)
    let uid = unsafe { libc::getuid() };
    let dir = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d).join("bhote"),
        _ if std::path::Path::new(&format!("/run/user/{uid}")).is_dir() => PathBuf::from(format!("/run/user/{uid}/bhote")),
        _ => PathBuf::from(format!("/tmp/bhote-mux-{uid}")),
    };
    dir.join("mux.sock")
}

/// Writes what fits into the socket right now and says how much (0: it is full). The socket is non-blocking (O_NONBLOCK, set when a client attaches):
/// a client that does not read must not hold the server up. (MSG_DONTWAIT is no help: macOS does not honour it for these sockets.)
pub fn send_nb(s: &std::os::unix::net::UnixStream, b: &[u8]) -> io::Result<usize> {
    if b.is_empty() { return Ok(0); }
    match (&*s).write(b) {
        Ok(n) => Ok(n),
        Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::Interrupted => Ok(0),
        Err(e) => Err(e),
    }
}

/// A reader for a non-blocking socket: waits with poll when there is nothing yet (so the thread of a client can block on it, while the server writes to the same socket).
pub struct PollRead<R>(pub R);
impl PollRead<io::BufReader<std::os::unix::net::UnixStream>> {
    pub fn wait_readable(&self) {
        use std::os::fd::AsRawFd;
        let mut p = libc::pollfd { fd: self.0.get_ref().as_raw_fd(), events: libc::POLLIN, revents: 0 };
        unsafe { libc::poll(&mut p, 1, 1000); }
    }
}
impl Read for PollRead<io::BufReader<std::os::unix::net::UnixStream>> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.0.read(buf) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::Interrupted => self.wait_readable(),
                r => return r,
            }
        }
    }
}

pub fn frame_bytes(t: u8, payload: &[u8]) -> Vec<u8> { let mut b = Vec::with_capacity(5 + payload.len()); b.push(t); b.extend_from_slice(&(payload.len() as u32).to_be_bytes()); b.extend_from_slice(payload); b }

/// frame = type:u8, len:u32 big endian, payload
pub fn write_frame(w: &mut impl Write, t: u8, payload: &[u8]) -> io::Result<()> {
    let mut b = Vec::with_capacity(5 + payload.len());
    b.push(t); b.extend_from_slice(&(payload.len() as u32).to_be_bytes()); b.extend_from_slice(payload);
    w.write_all(&b)
}

pub fn read_frame(r: &mut impl Read) -> io::Result<(u8, Vec<u8>)> {
    let mut h = [0u8; 5];
    r.read_exact(&mut h)?;
    let n = u32::from_be_bytes([h[1], h[2], h[3], h[4]]) as usize;
    if n > 64 << 20 { return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large")); }
    let mut p = vec![0u8; n];
    r.read_exact(&mut p)?;
    Ok((h[0], p))
}

pub fn resize_payload(cols: u16, rows: u16) -> Vec<u8> { let mut v = cols.to_be_bytes().to_vec(); v.extend_from_slice(&rows.to_be_bytes()); v }
pub fn parse_resize(p: &[u8]) -> Option<(u16, u16)> { if p.len() == 4 { Some((u16::from_be_bytes([p[0], p[1]]), u16::from_be_bytes([p[2], p[3]]))) } else { None } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_round_trip() {
        let mut b = Vec::new();
        write_frame(&mut b, b'O', b"hello").unwrap();
        write_frame(&mut b, b'R', &resize_payload(120, 40)).unwrap();
        let mut r = &b[..];
        assert_eq!(read_frame(&mut r).unwrap(), (b'O', b"hello".to_vec()));
        let (t, p) = read_frame(&mut r).unwrap();
        assert_eq!((t, parse_resize(&p)), (b'R', Some((120, 40))));
        assert!(read_frame(&mut r).is_err());
    }
}
