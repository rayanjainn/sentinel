//! `<sys/proc_info.h>` structures that `libc` does not define. Layouts follow the XNU headers;
//! every call checks the byte count the kernel reports against these sizes.

#![allow(non_camel_case_types)]

use libc::{c_int, off_t, vinfo_stat};

pub const PROC_PIDFDVNODEPATHINFO: c_int = 2;

pub const MAXPATHLEN: usize = 1024;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct proc_fileinfo {
    pub fi_openflags: u32,
    pub fi_status: u32,
    pub fi_offset: off_t,
    pub fi_type: i32,
    pub fi_guardflags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct vnode_info {
    pub vi_stat: vinfo_stat,
    pub vi_type: c_int,
    pub vi_pad: c_int,
    pub vi_fsid: [i32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct vnode_info_path {
    pub vip_vi: vnode_info,
    pub vip_path: [libc::c_char; MAXPATHLEN],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct vnode_fdinfowithpath {
    pub pfi: proc_fileinfo,
    pub pvip: vnode_info_path,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_match_xnu_headers() {
        assert_eq!(std::mem::size_of::<vinfo_stat>(), 136);
        assert_eq!(std::mem::size_of::<proc_fileinfo>(), 24);
        assert_eq!(std::mem::size_of::<vnode_info>(), 152);
        assert_eq!(std::mem::size_of::<vnode_fdinfowithpath>(), 1200);
    }
}
