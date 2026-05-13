use super::config::PPP_PARSE_BUFFER;

const PPP_IPCP_PROTOCOL: [u8; 2] = [0x80, 0x21];
const PPP_CONFIGURE_REQUEST: u8 = 1;
const IPCP_OPTION_IP_ADDRESS: u8 = 3;
const PPP_FCS_INITIAL: u16 = 0xffff;
const PPP_FCS_POLY: u16 = 0x8408;

pub(super) const IPCP_FALLBACK_FRAME_MAX: usize = 64;

pub(super) struct IpcpFallback {
    frame: [u8; IPCP_FALLBACK_FRAME_MAX],
    frame_len: usize,
    attempts: u8,
}

impl IpcpFallback {
    pub(super) const fn new() -> Self {
        Self {
            frame: [0; IPCP_FALLBACK_FRAME_MAX],
            frame_len: 0,
            attempts: 0,
        }
    }

    pub(super) fn reset(&mut self) {
        self.frame_len = 0;
        self.attempts = 0;
    }

    pub(super) fn cache_from_ppp_frame(&mut self, frame: &[u8]) -> bool {
        let mut decoded = [0u8; PPP_PARSE_BUFFER];
        let used = decode_ppp_bytes(frame, &mut decoded);
        if used < 10 {
            return false;
        }

        for i in 0..(used.saturating_sub(9)) {
            if decoded[i..].get(..3)
                != Some(&[
                    PPP_IPCP_PROTOCOL[0],
                    PPP_IPCP_PROTOCOL[1],
                    PPP_CONFIGURE_REQUEST,
                ])
            {
                continue;
            }

            let id = decoded[i + 3];
            let plen = usize::from(u16::from_be_bytes([decoded[i + 4], decoded[i + 5]]));
            let Some(options_start) = i.checked_add(6) else {
                return false;
            };
            let Some(frame_end) = i.checked_add(2 + plen) else {
                return false;
            };
            if options_start > used || frame_end > used || options_start > frame_end {
                return false;
            }

            let address = ipcp_address_from_options(&decoded[options_start..frame_end]);
            let len = build_ipcp_ip_only_request(id, address, &mut self.frame);
            if len == 0 {
                return false;
            }

            self.frame_len = len;
            self.attempts = 0;
            return true;
        }

        false
    }

    pub(super) fn next_frame(&mut self, out: &mut [u8; IPCP_FALLBACK_FRAME_MAX]) -> Option<usize> {
        if self.frame_len == 0 || self.attempts >= 6 {
            return None;
        }

        self.attempts = self.attempts.saturating_add(1);
        let len = self.frame_len;
        out[..len].copy_from_slice(&self.frame[..len]);
        Some(len)
    }
}

fn decode_ppp_bytes(input: &[u8], out: &mut [u8]) -> usize {
    let mut i = 0usize;
    let mut used = 0usize;

    while i < input.len() && used < out.len() {
        let b = input[i];
        i += 1;

        if b == 0x7e {
            continue;
        }

        if b == 0x7d {
            if i >= input.len() {
                break;
            }
            out[used] = input[i] ^ 0x20;
            i += 1;
            used += 1;
            continue;
        }

        out[used] = b;
        used += 1;
    }

    used
}

fn ipcp_address_from_options(mut options: &[u8]) -> [u8; 4] {
    while options.len() >= 2 {
        let code = options[0];
        let len = usize::from(options[1]);
        if len < 2 || len > options.len() {
            return [0, 0, 0, 0];
        }
        if code == IPCP_OPTION_IP_ADDRESS && len == 6 {
            return [options[2], options[3], options[4], options[5]];
        }
        options = &options[len..];
    }

    [0, 0, 0, 0]
}

fn build_ipcp_ip_only_request(id: u8, address: [u8; 4], out: &mut [u8]) -> usize {
    let payload = [
        0xff,
        0x03,
        PPP_IPCP_PROTOCOL[0],
        PPP_IPCP_PROTOCOL[1],
        PPP_CONFIGURE_REQUEST,
        id,
        0x00,
        0x0a,
        IPCP_OPTION_IP_ADDRESS,
        0x06,
        address[0],
        address[1],
        address[2],
        address[3],
    ];
    let fcs = ppp_fcs(&payload);

    let mut used = 0usize;
    if !push_ppp_raw(out, &mut used, 0x7e) {
        return 0;
    }
    for byte in payload {
        if !push_ppp_escaped(out, &mut used, byte) {
            return 0;
        }
    }
    for byte in fcs.to_le_bytes() {
        if !push_ppp_escaped(out, &mut used, byte) {
            return 0;
        }
    }
    if !push_ppp_raw(out, &mut used, 0x7e) {
        return 0;
    }

    used
}

fn ppp_fcs(bytes: &[u8]) -> u16 {
    let mut fcs = PPP_FCS_INITIAL;
    for &byte in bytes {
        fcs ^= u16::from(byte);
        for _ in 0..8 {
            if (fcs & 1) != 0 {
                fcs = (fcs >> 1) ^ PPP_FCS_POLY;
            } else {
                fcs >>= 1;
            }
        }
    }

    !fcs
}

fn push_ppp_raw(out: &mut [u8], used: &mut usize, byte: u8) -> bool {
    if *used >= out.len() {
        return false;
    }

    out[*used] = byte;
    *used += 1;
    true
}

fn push_ppp_escaped(out: &mut [u8], used: &mut usize, byte: u8) -> bool {
    if byte < 0x20 || matches!(byte, 0x7d | 0x7e) {
        push_ppp_raw(out, used, 0x7d) && push_ppp_raw(out, used, byte ^ 0x20)
    } else {
        push_ppp_raw(out, used, byte)
    }
}
