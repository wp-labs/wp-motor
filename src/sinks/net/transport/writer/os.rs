use super::NetWriter;
use super::Transport;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

impl NetWriter {
    /// Return Some(pending_bytes) if supported, None if unsupported, Err on syscall error.
    pub(super) fn os_sendq_len(&self) -> Result<Option<usize>, std::io::Error> {
        #[cfg(unix)]
        {
            if let Transport::Tcp(stream) = &self.transport {
                let fd = stream.as_raw_fd();
                // Linux: ioctl TIOCOUTQ (get number of unsent bytes in socket send queue)
                #[cfg(target_os = "linux")]
                unsafe {
                    let mut outq: libc::c_int = 0;
                    let rc = libc::ioctl(fd, libc::TIOCOUTQ, &mut outq);
                    if rc == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    return Ok(Some(outq as usize));
                }
                // Darwin/BSD: getsockopt(SOL_SOCKET, SO_NWRITE)
                #[cfg(any(
                    target_os = "macos",
                    target_os = "ios",
                    target_os = "freebsd",
                    target_os = "openbsd",
                    target_os = "netbsd"
                ))]
                unsafe {
                    let mut n: libc::c_int = 0;
                    let mut len: libc::socklen_t = std::mem::size_of::<libc::c_int>() as _;
                    #[allow(non_upper_case_globals)]
                    let opt = libc::SO_NWRITE;
                    let rc = libc::getsockopt(
                        fd,
                        libc::SOL_SOCKET,
                        opt,
                        &mut n as *mut _ as *mut _,
                        &mut len,
                    );
                    if rc == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    return Ok(Some(n as usize));
                }
            }
        }
        Ok(None)
    }

    /// 返回 SO_SNDBUF（内核发送缓冲区大小，字节）。不支持的平台返回 None。
    pub(super) fn os_sndbuf_size(&self) -> Result<Option<usize>, std::io::Error> {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            if let Transport::Tcp(stream) = &self.transport {
                let fd = stream.as_raw_fd();
                unsafe {
                    let mut n: libc::c_int = 0;
                    let mut len: libc::socklen_t = std::mem::size_of::<libc::c_int>() as _;
                    let rc = libc::getsockopt(
                        fd,
                        libc::SOL_SOCKET,
                        libc::SO_SNDBUF,
                        &mut n as *mut _ as *mut _,
                        &mut len,
                    );
                    if rc == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    return Ok(Some(n as usize));
                }
            }
        }
        Ok(None)
    }

    /// 获取（并缓存）SO_SNDBUF；测试可覆写。
    pub(super) fn get_cached_sndbuf(&mut self) -> Option<usize> {
        #[cfg(test)]
        {
            if let Some(v) = self.sndbuf_override {
                return Some(v);
            }
        }
        if let Some(v) = self.cached_sndbuf {
            return Some(v);
        }
        if let Ok(Some(v)) = self.os_sndbuf_size() {
            self.cached_sndbuf = Some(v);
            return Some(v);
        }
        None
    }

    /// 读取 send-queue 与 sndbuf（测试下支持覆盖）。
    pub(super) fn peek_kernel_queues(&mut self) -> (Option<usize>, Option<usize>) {
        let pending = {
            #[cfg(test)]
            {
                if let Some(p) = self.pending_override {
                    Some(p)
                } else {
                    self.os_sendq_len().unwrap_or(None)
                }
            }
            #[cfg(not(test))]
            {
                self.os_sendq_len().unwrap_or(None)
            }
        };
        #[allow(unused_mut)]
        let mut sndbuf = self.get_cached_sndbuf();
        #[cfg(test)]
        {
            if self.sndbuf_override.is_some() {
                sndbuf = self.sndbuf_override;
            }
        }
        (pending, sndbuf)
    }
}
