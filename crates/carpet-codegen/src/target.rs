#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    LinuxX86_64,
    MacOSX86_64,
    WindowsX86_64,
}

impl Target {
    pub fn sys_write(&self) -> u64 {
        match self {
            Target::LinuxX86_64 => 1,
            Target::MacOSX86_64 => 0x2000004,
            Target::WindowsX86_64 => 0,
        }
    }

    pub fn sys_exit(&self) -> u64 {
        match self {
            Target::LinuxX86_64 => 60,
            Target::MacOSX86_64 => 0x2000001,
            Target::WindowsX86_64 => 0,
        }
    }

    pub fn from_host() -> Self {
        #[cfg(target_os = "linux")]
        {
            Target::LinuxX86_64
        }
        #[cfg(target_os = "macos")]
        {
            Target::MacOSX86_64
        }
        #[cfg(target_os = "windows")]
        {
            Target::WindowsX86_64
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Target::LinuxX86_64
        }
    }
}
