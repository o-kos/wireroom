use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    os::fd::{AsRawFd, FromRawFd},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    time::{Duration, Instant},
};

pub struct PacketProcess {
    pub child: Child,
    pub input: ChildStdin,
    pub output: BufReader<File>,
}
impl PacketProcess {
    pub fn new(root: &Path) -> Self {
        let mut fds = [-1; 2];
        // SAFETY: fds has storage for both newly owned pipe descriptors.
        assert_eq!(
            // SAFETY: fds has room for two descriptors.
            unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_DIRECT | libc::O_CLOEXEC) },
            0
        );
        // SAFETY: successful pipe2 returned these distinct owned descriptors.
        let read = unsafe { File::from_raw_fd(fds[0]) };
        // SAFETY: successful pipe2 returned this owned write descriptor.
        let write = unsafe { File::from_raw_fd(fds[1]) };
        // One page holds exactly one packet, so each event requires reader credit.
        // SAFETY: write is a live pipe descriptor; this fcntl takes an integer size.
        assert!(unsafe { libc::fcntl(write.as_raw_fd(), libc::F_SETPIPE_SZ, 4096) } > 0);
        let mut child = Command::new(env!("CARGO_BIN_EXE_dmd"))
            .args(["run", "--message-format", "json"])
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(write)
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        Self {
            child,
            input,
            output: BufReader::new(read),
        }
    }
    pub fn next(&mut self) -> Value {
        if self.output.buffer().is_empty() {
            let mut poll = libc::pollfd {
                fd: self.output.get_ref().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: poll points to one initialized descriptor entry.
            assert_eq!(unsafe { libc::poll(&mut poll, 1, 15_000) }, 1);
        }
        let mut line = String::new();
        assert!(self.output.read_line(&mut line).unwrap() > 0);
        serde_json::from_str(&line).unwrap()
    }
    pub fn send(&mut self, command: &Value) {
        writeln!(self.input, "{command}").unwrap();
        self.input.flush().unwrap();
    }
    pub fn consumed(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut bytes: libc::c_int = 0;
            // SAFETY: FIONREAD writes one c_int; input is a live pipe endpoint.
            assert_eq!(
                // SAFETY: bytes is c_int output storage for this live pipe.
                unsafe { libc::ioctl(self.input.as_raw_fd(), libc::FIONREAD, &mut bytes) },
                0
            );
            if bytes == 0 {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "stdin worker did not consume command"
            );
            std::thread::yield_now();
        }
    }
}
pub fn wait(child: &mut Child) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "process did not terminate and join workers"
        );
        std::thread::yield_now();
    }
}
impl Drop for PacketProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
