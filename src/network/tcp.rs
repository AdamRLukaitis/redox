use core::clone::Clone;
use core::mem::size_of;
use core::option::Option;

use common::debug::*;
use common::memory::*;
use common::random::*;
use common::string::*;
use common::vector::*;

use network::common::*;
use network::http::*;

#[derive(Copy, Clone)]
pub struct TCPHeader {
    pub src: n16,
    pub dst: n16,
    pub sequence: n32,
    pub ack_num: n32,
    pub flags: n16,
    pub window_size: n16,
    pub checksum: Checksum,
    pub urgent_pointer: n16
}

pub struct TCP {
    header: TCPHeader,
    options: Vector<u8>,
    data: Vector<u8>,
    src_ip: IPv4Addr,
    dst_ip: IPv4Addr
}

impl ToBytes for TCP {
    fn to_bytes(&self) -> Vector<u8> {
        unsafe{
            let header_ptr: *const TCPHeader = &self.header;
            Vector::<u8>::from_raw(header_ptr as *const u8, size_of::<TCPHeader>()) + self.options.clone() + self.data.clone()
        }
    }
}

const TCP_FIN: u16 = 1;
const TCP_SYN: u16 = 1 << 1;
const TCP_RST: u16 = 1 << 2;
const TCP_PSH: u16 = 1 << 3;
const TCP_ACK: u16 = 1 << 4;

#[allow(trivial_casts)]
impl Response for TCP {
    fn respond(&self) -> Vector<Vector<u8>> {
        d("            ");
        self.d();
        dl();

        let allow;
        match self.header.dst.get() {
            80 => allow = true,
            _ => allow = false
        }

        if allow {
            if self.header.flags.get() & TCP_SYN != 0 {
                d("            TCP SYN\n");
                let mut ret = TCP {
                    header: self.header,
                    options: self.options.clone(),
                    data: Vector::new(),
                    src_ip: IP_ADDR,
                    dst_ip: self.src_ip
                };

                ret.header.src = self.header.dst;
                ret.header.dst = self.header.src;
                ret.header.flags.set(self.header.flags.get() | TCP_ACK);
                ret.header.ack_num.set(self.header.sequence.get() + 1);
                ret.header.sequence.set(rand() as u32);

                unsafe{
                    ret.header.checksum.data = 0;

                    let proto = n16::new(0x06);
                    let segment_len = n16::new((size_of::<TCPHeader>() + ret.options.len() + ret.data.len()) as u16);
                    ret.header.checksum.data = Checksum::compile(
                        Checksum::sum((&ret.src_ip as *const IPv4Addr) as usize, size_of::<IPv4Addr>()) +
                        Checksum::sum((&ret.dst_ip as *const IPv4Addr) as usize, size_of::<IPv4Addr>()) +
                        Checksum::sum((&proto as *const n16) as usize, size_of::<n16>()) +
                        Checksum::sum((&segment_len as *const n16) as usize, size_of::<n16>()) +
                        Checksum::sum((&ret.header as *const TCPHeader) as usize, size_of::<TCPHeader>()) +
                        Checksum::sum(ret.options.data as usize, ret.options.len()) +
                        Checksum::sum(ret.data.data as usize, ret.data.len())
                    );
                }

                return Vector::from_value(ret.to_bytes());
            }else if self.header.flags.get() & TCP_PSH != 0{
                d("            TCP PSH\n");
                //Send TCP_ACK_PSH_FIN in one statement
                {
                    let mut ret = TCP {
                        header: self.header,
                        options: self.options.clone(),
                        data: Vector::new(),
                        src_ip: IP_ADDR,
                        dst_ip: self.src_ip
                    };

                    ret.header.src = self.header.dst;
                    ret.header.dst = self.header.src;
                    ret.header.flags.set(self.header.flags.get() | TCP_FIN);
                    ret.header.ack_num.set(self.header.sequence.get() + self.data.len() as u32);
                    ret.header.sequence.set(self.header.ack_num.get());

                    match self.header.dst.get() {
                        80 => {
                            // TODO: More efficient method
                            let html = http_response(String::from_c_slice(self.data.as_slice()));
                            unsafe{
                                let html_ptr = html.to_c_str();
                                ret.data = Vector::from_raw(html_ptr, html.len());
                                unalloc(html_ptr as usize);
                            }
                        },
                        _ => ()
                    }

                    unsafe{
                        ret.header.checksum.data = 0;

                        let proto = n16::new(0x06);
                        let segment_len = n16::new((size_of::<TCPHeader>() + ret.options.len() + ret.data.len()) as u16);
                        ret.header.checksum.data = Checksum::compile(
                            Checksum::sum((&ret.src_ip as *const IPv4Addr) as usize, size_of::<IPv4Addr>()) +
                            Checksum::sum((&ret.dst_ip as *const IPv4Addr) as usize, size_of::<IPv4Addr>()) +
                            Checksum::sum((&proto as *const n16) as usize, size_of::<n16>()) +
                            Checksum::sum((&segment_len as *const n16) as usize, size_of::<n16>()) +
                            Checksum::sum((&ret.header as *const TCPHeader) as usize, size_of::<TCPHeader>()) +
                            Checksum::sum(ret.options.data as usize, ret.options.len()) +
                            Checksum::sum(ret.data.data as usize, ret.data.len())
                        );
                    }

                    return Vector::from_value(ret.to_bytes());
                }
            }
        }else{
            d("            TCP RST TODO\n");
        }

        return Vector::new();
    }
}

impl TCP {
    pub fn from_bytes_ipv4(bytes: Vector<u8>, src_ip: IPv4Addr, dst_ip: IPv4Addr) -> Option<TCP> {
        if bytes.len() >= size_of::<TCPHeader>() {
            unsafe {
                let header = *(bytes.data as *const TCPHeader);
                let header_len = ((header.flags.get() & 0xF000) >> 10) as usize;

                return Option::Some(TCP {
                    header: header,
                    options: bytes.sub(size_of::<TCPHeader>(), header_len - size_of::<TCPHeader>()),
                    data: bytes.sub(header_len, bytes.len() - header_len),
                    src_ip: src_ip,
                    dst_ip: dst_ip
                });
            }
        }
        return Option::None;
    }

    pub fn d(&self){
        d("TCP from ");
        dd(self.header.src.get() as usize);
        d(" to ");
        dd(self.header.dst.get() as usize);
        d(" options ");
        dd(self.options.len());
        d(" data ");
        dd(self.data.len());
    }
}