//! `<sys/proc_info.h>` structures that `libc` does not define. Layouts follow the XNU headers;
//! every call checks the byte count the kernel reports against these sizes.

#![allow(non_camel_case_types, dead_code)]

use libc::{c_int, c_short, c_ushort, off_t, vinfo_stat};

pub const PROC_PIDFDVNODEPATHINFO: c_int = 2;
pub const PROC_PIDFDSOCKETINFO: c_int = 3;

pub const SOCKINFO_IN: c_int = 1;
pub const SOCKINFO_TCP: c_int = 2;

pub const INI_IPV4: u8 = 0x1;
pub const INI_IPV6: u8 = 0x2;

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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct sockbuf_info {
    pub sbi_cc: u32,
    pub sbi_hiwat: u32,
    pub sbi_mbcnt: u32,
    pub sbi_mbmax: u32,
    pub sbi_lowat: u32,
    pub sbi_flags: c_short,
    pub sbi_timeo: c_short,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct in4in6_addr {
    pub i46a_pad32: [u32; 3],
    pub i46a_addr4: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union in_addr_4_6 {
    pub ina_46: in4in6_addr,
    pub ina_6: [u8; 16],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct in_sockinfo_v6 {
    pub in6_hlim: u8,
    pub in6_cksum: c_int,
    pub in6_ifindex: c_ushort,
    pub in6_hops: c_short,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct in_sockinfo {
    pub insi_fport: c_int,
    pub insi_lport: c_int,
    pub insi_gencnt: u64,
    pub insi_flags: u32,
    pub insi_flow: u32,
    pub insi_vflag: u8,
    pub insi_ip_ttl: u8,
    pub rfu_1: u32,
    pub insi_faddr: in_addr_4_6,
    pub insi_laddr: in_addr_4_6,
    pub insi_v4_tos: u8,
    pub insi_v6: in_sockinfo_v6,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tcp_sockinfo {
    pub tcpsi_ini: in_sockinfo,
    pub tcpsi_state: c_int,
    pub tcpsi_timer: [c_int; 4],
    pub tcpsi_mss: c_int,
    pub tcpsi_flags: u32,
    pub rfu_1: u32,
    pub tcpsi_tp: u64,
}

/// The protocol union's largest member is `un_sockinfo` (2 × u64 + 2 × 255-byte address unions).
pub const SOCKET_PROTO_UNION_BYTES: usize = 528;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct socket_info {
    pub soi_stat: vinfo_stat,
    pub soi_so: u64,
    pub soi_pcb: u64,
    pub soi_type: c_int,
    pub soi_protocol: c_int,
    pub soi_family: c_int,
    pub soi_options: c_short,
    pub soi_linger: c_short,
    pub soi_state: c_short,
    pub soi_qlen: c_short,
    pub soi_incqlen: c_short,
    pub soi_qlimit: c_short,
    pub soi_timeo: c_short,
    pub soi_error: c_ushort,
    pub soi_oobmark: u32,
    pub soi_rcv: sockbuf_info,
    pub soi_snd: sockbuf_info,
    pub soi_kind: c_int,
    pub rfu_1: u32,
    /// Aligned storage for the `soi_proto` union; read through `tcp()` / `inet()`.
    pub soi_proto: [u64; SOCKET_PROTO_UNION_BYTES / 8],
}

impl socket_info {
    pub fn inet(&self) -> in_sockinfo {
        // SAFETY: the union storage is at least as large and as aligned as in_sockinfo; the
        // caller checks soi_kind before interpreting it.
        unsafe { std::ptr::read(self.soi_proto.as_ptr().cast::<in_sockinfo>()) }
    }

    pub fn tcp(&self) -> tcp_sockinfo {
        // SAFETY: as above, for tcp_sockinfo when soi_kind == SOCKINFO_TCP.
        unsafe { std::ptr::read(self.soi_proto.as_ptr().cast::<tcp_sockinfo>()) }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct socket_fdinfo {
    pub pfi: proc_fileinfo,
    pub psi: socket_info,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_match_xnu_headers() {
        assert_eq!(std::mem::size_of::<vinfo_stat>(), 136);
        assert_eq!(std::mem::size_of::<proc_fileinfo>(), 24);
        assert_eq!(std::mem::size_of::<in_sockinfo>(), 80);
        assert_eq!(std::mem::size_of::<tcp_sockinfo>(), 120);
        assert_eq!(std::mem::size_of::<socket_info>(), 768);
        assert_eq!(std::mem::size_of::<socket_fdinfo>(), 792);
        assert_eq!(std::mem::size_of::<vnode_fdinfowithpath>(), 1200);
    }
}
