#![forbid(unsafe_code)]
//! Interpreter for the compiled `line6_variax_process_message` prefix.
//!
//! The instruction arrays are Linux object code for virtual addresses
//! `[0x160, 0x240)`. Upstream `sound/usb/line6/variax.c` is
//! `SPDX-License-Identifier: GPL-2.0-only`, Copyright (C) 2004-2010 Markus
//! Grabner, pinned at commit `497791a0a96b866d90add64041c47acdff49cae6`.
//! The notice and license are under `third_party/`. These arrays are test
//! reference bytes, not a runtime dependency. Message buffers passed in are
//! separate synthetic inputs. External calls, tail jumps, and the relocated
//! cold branch stop or return here. This does not load a module or enter
//! kernel code.

use crate::linux_transport::{self, INIT_DONE_PREFIX, STARTUP_ACTIVATE, STARTUP_STATE_OFFSET};

const PREFIX_BASE: u64 = 0x160;
const DEVICE_BASE: u64 = 0x10_0000;
const DEVICE_SIZE: usize = 0x3000;
const MSG_BASE: u64 = 0x20_0000;
const STACK_BASE: u64 = 0x30_0000;
const STACK_SIZE: usize = 0x400;
const MSG_PTR_OFFSET: usize = 0x50;
const WORK_DISPLACEMENT: u64 = 0x20b0;
/// `movl $0x2000, %edi` before `queue_delayed_work_on`.
const QUEUE_CPU: u64 = 0x2000;
const STEP_LIMIT: u32 = 80;

const RAX: usize = 0;
const RCX: usize = 1;
const RDX: usize = 2;
const RSP: usize = 4;
const RSI: usize = 6;
const RDI: usize = 7;

/// SHA-256 of the unpatched `line6_variax_process_message` bytes below.
pub const UNPATCHED_PREFIX_SHA256: &str =
    "68dea14f9600a5c5cb820c35f2a9ab3a65e71f112b0dbc5017d8e0146ad3d7a1";
/// SHA-256 of the patched function bytes below.
pub const PATCHED_PREFIX_SHA256: &str =
    "e9ef1f53b7e6ce3c52ac39581dd4fe1bdccd634d56b4358c505916b301cd1a6b";

const PREFIX_NOTICE: &str = include_str!("../third_party/notices/linux-variax-prefix-NOTICE.txt");
const GPL_TEXT: &str = include_str!("../third_party/notices/GPL-2.0-only.txt");

/// What a relocated site does. None of these transfer into kernel code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum External {
    /// `call __fentry__`. Recorded, then the prefix continues.
    Fentry,
    /// `je` to the cold copy. Taken means stop. Not taken skips the branch.
    ColdBranch,
    /// `jmp __x86_return_thunk`.
    ReturnThunk,
    /// `call cancel_delayed_work`. Recorded, then the prefix continues.
    CancelDelayedWork,
    /// `movq system_percpu_wq(%rip), %rsi`. The pointer is not loaded.
    WorkqueuePointer,
    /// `jmp queue_delayed_work_on`.
    QueueDelayedWork,
}

const UNPATCHED_EXTERNALS: &[(u64, External)] = &[
    (0x164, External::Fentry),
    (0x178, External::ColdBranch),
    (0x17e, External::ReturnThunk),
    (0x1a4, External::ReturnThunk),
    (0x1d1, External::CancelDelayedWork),
    (0x1d6, External::WorkqueuePointer),
    (0x1ef, External::QueueDelayedWork),
    (0x213, External::CancelDelayedWork),
    (0x223, External::WorkqueuePointer),
    (0x22e, External::QueueDelayedWork),
];

const PATCHED_EXTERNALS: &[(u64, External)] = &[
    (0x164, External::Fentry),
    (0x178, External::ColdBranch),
    (0x17e, External::ReturnThunk),
    (0x1a4, External::ReturnThunk),
    (0x1c8, External::CancelDelayedWork),
    (0x1d8, External::WorkqueuePointer),
    (0x1e3, External::QueueDelayedWork),
    (0x21c, External::CancelDelayedWork),
    (0x221, External::WorkqueuePointer),
    (0x23a, External::QueueDelayedWork),
];

/// Unpatched Linux object code for `[0x160, 0x240)`. See
/// `third_party/notices/linux-variax-prefix-NOTICE.txt`.
const UNPATCHED_PREFIX: [u8; 224] = [
    0xf3, 0x0f, 0x1e, 0xfa, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x47, 0x50, 0x0f, 0xb6, 0x10,
    0x80, 0xfa, 0xf0, 0x74, 0x0e, 0x80, 0xfa, 0xff, 0x0f, 0x84, 0x00, 0x00, 0x00, 0x00, 0xe9, 0x00,
    0x00, 0x00, 0x00, 0x48, 0xba, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x07, 0x48, 0x83, 0xec,
    0x08, 0x48, 0x39, 0x50, 0x01, 0x74, 0x12, 0x81, 0x78, 0x01, 0x00, 0x01, 0x0c, 0x07, 0x74, 0x54,
    0x48, 0x83, 0xc4, 0x08, 0xe9, 0x00, 0x00, 0x00, 0x00, 0x83, 0x78, 0x08, 0x07, 0x75, 0xe8, 0x8b,
    0x87, 0x28, 0x21, 0x00, 0x00, 0x85, 0xc0, 0x7f, 0xe7, 0xc7, 0x87, 0x28, 0x21, 0x00, 0x00, 0x01,
    0x00, 0x00, 0x00, 0x48, 0x8d, 0x97, 0xb0, 0x20, 0x00, 0x00, 0x48, 0x89, 0xd7, 0x48, 0x89, 0x14,
    0x24, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x35, 0x00, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x14,
    0x24, 0xb9, 0x64, 0x00, 0x00, 0x00, 0xbf, 0x00, 0x20, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, 0xe9,
    0x00, 0x00, 0x00, 0x00, 0x66, 0x81, 0x78, 0x05, 0x00, 0x6b, 0x75, 0xa4, 0x83, 0xbf, 0x28, 0x21,
    0x00, 0x00, 0x01, 0x7f, 0x9b, 0x48, 0x8d, 0x97, 0xb0, 0x20, 0x00, 0x00, 0x48, 0x89, 0xd7, 0x48,
    0x89, 0x14, 0x24, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x14, 0x24, 0x31, 0xc9, 0xbf, 0x00,
    0x20, 0x00, 0x00, 0x48, 0x8b, 0x35, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, 0xe9, 0x00,
    0x00, 0x00, 0x00, 0x66, 0x90, 0x66, 0x66, 0x2e, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Patched Linux object code for the same `[0x160, 0x240)` window.
const PATCHED_PREFIX: [u8; 224] = [
    0xf3, 0x0f, 0x1e, 0xfa, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x47, 0x50, 0x0f, 0xb6, 0x10,
    0x80, 0xfa, 0xf0, 0x74, 0x0e, 0x80, 0xfa, 0xff, 0x0f, 0x84, 0x00, 0x00, 0x00, 0x00, 0xe9, 0x00,
    0x00, 0x00, 0x00, 0x48, 0xba, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x07, 0x48, 0x83, 0xec,
    0x08, 0x48, 0x39, 0x50, 0x01, 0x74, 0x51, 0x81, 0x78, 0x01, 0x00, 0x01, 0x0c, 0x07, 0x74, 0x09,
    0x48, 0x83, 0xc4, 0x08, 0xe9, 0x00, 0x00, 0x00, 0x00, 0x66, 0x81, 0x78, 0x05, 0x00, 0x6b, 0x75,
    0xef, 0x83, 0xbf, 0x28, 0x21, 0x00, 0x00, 0x01, 0x7f, 0xe6, 0x48, 0x8d, 0x97, 0xb0, 0x20, 0x00,
    0x00, 0x48, 0x89, 0xd7, 0x48, 0x89, 0x14, 0x24, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x14,
    0x24, 0x31, 0xc9, 0xbf, 0x00, 0x20, 0x00, 0x00, 0x48, 0x8b, 0x35, 0x00, 0x00, 0x00, 0x00, 0x48,
    0x83, 0xc4, 0x08, 0xe9, 0x00, 0x00, 0x00, 0x00, 0x80, 0x78, 0x09, 0x00, 0x75, 0xa9, 0xf6, 0x40,
    0x0a, 0xfb, 0x75, 0xa3, 0x80, 0x78, 0x0b, 0x00, 0x75, 0x9d, 0x8b, 0x87, 0x28, 0x21, 0x00, 0x00,
    0x85, 0xc0, 0x7f, 0x9c, 0xc7, 0x87, 0x28, 0x21, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x48, 0x8d,
    0x97, 0xb0, 0x20, 0x00, 0x00, 0x48, 0x89, 0xd7, 0x48, 0x89, 0x14, 0x24, 0xe8, 0x00, 0x00, 0x00,
    0x00, 0x48, 0x8b, 0x35, 0x00, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x14, 0x24, 0xb9, 0x64, 0x00, 0x00,
    0x00, 0xbf, 0x00, 0x20, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, 0xe9, 0x00, 0x00, 0x00, 0x00, 0x90,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    ReturnThunk,
    ColdPath,
    Queue { delay: u64, cpu: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stub {
    Fentry,
    CancelDelayedWork,
    WorkqueuePointer,
}

struct Image {
    bytes: &'static [u8],
    externals: &'static [(u64, External)],
}

const UNPATCHED: Image = Image {
    bytes: &UNPATCHED_PREFIX,
    externals: UNPATCHED_EXTERNALS,
};
const PATCHED: Image = Image {
    bytes: &PATCHED_PREFIX,
    externals: PATCHED_EXTERNALS,
};

struct Outcome {
    startup: i32,
    stop: Stop,
    stubs: Vec<Stub>,
}

#[derive(Debug, PartialEq, Eq)]
struct Reject(&'static str);

struct Machine<'a> {
    image: &'a Image,
    rip: u64,
    regs: [u64; 8],
    zf: bool,
    sf: bool,
    of: bool,
    device: Vec<u8>,
    message: Vec<u8>,
    stack: Vec<u8>,
    stubs: Vec<Stub>,
    steps: u32,
}

impl<'a> Machine<'a> {
    fn new(image: &'a Image, message: &[u8], startup: i32) -> Result<Self, Reject> {
        if message.len() < 12 {
            return Err(Reject("message shorter than the comparator reads"));
        }
        let mut device = vec![0u8; DEVICE_SIZE];
        device[MSG_PTR_OFFSET..MSG_PTR_OFFSET + 8].copy_from_slice(&MSG_BASE.to_le_bytes());
        let startup_at = usize::try_from(STARTUP_STATE_OFFSET).map_err(|_| Reject("offset"))?;
        device[startup_at..startup_at + 4].copy_from_slice(&startup.to_le_bytes());
        let mut machine = Self {
            image,
            rip: PREFIX_BASE,
            regs: [0; 8],
            zf: false,
            sf: false,
            of: false,
            device,
            message: message.to_vec(),
            stack: vec![0u8; STACK_SIZE],
            stubs: Vec::new(),
            steps: 0,
        };
        machine.regs[RDI] = DEVICE_BASE;
        machine.regs[RSP] = STACK_BASE + STACK_SIZE as u64;
        Ok(machine)
    }

    fn run(mut self) -> Result<Outcome, Reject> {
        let stop = loop {
            if self.steps >= STEP_LIMIT {
                return Err(Reject("prefix exceeded the step limit"));
            }
            self.steps += 1;
            if let Some(stop) = self.step()? {
                break stop;
            }
        };
        Ok(Outcome {
            startup: self.startup(),
            stop,
            stubs: self.stubs,
        })
    }

    fn startup(&self) -> i32 {
        let at = STARTUP_STATE_OFFSET as usize;
        i32::from_le_bytes(
            self.device[at..at + 4]
                .try_into()
                .expect("four startup bytes"),
        )
    }

    fn step(&mut self) -> Result<Option<Stop>, Reject> {
        if let Some(kind) = self.external_at(self.rip) {
            return self.external(kind);
        }
        if self.rip >= PREFIX_BASE + self.image.bytes.len() as u64 {
            return Err(Reject("execution left the compiled prefix"));
        }
        self.decode()
    }

    fn external_at(&self, pc: u64) -> Option<External> {
        self.image
            .externals
            .iter()
            .find(|(at, _)| *at == pc)
            .map(|(_, kind)| *kind)
    }

    fn external(&mut self, kind: External) -> Result<Option<Stop>, Reject> {
        match kind {
            External::Fentry => {
                self.expect_bytes(&[0xe8])?;
                self.skip(4)?;
                self.stubs.push(Stub::Fentry);
                Ok(None)
            }
            External::CancelDelayedWork => {
                self.expect_bytes(&[0xe8])?;
                if self.regs[RDI] != DEVICE_BASE + WORK_DISPLACEMENT {
                    return Err(Reject(
                        "cancel stub did not see the compiled work displacement",
                    ));
                }
                self.skip(4)?;
                self.stubs.push(Stub::CancelDelayedWork);
                Ok(None)
            }
            External::WorkqueuePointer => {
                self.expect_bytes(&[0x48, 0x8b, 0x35])?;
                self.skip(4)?;
                self.regs[RSI] = 0;
                self.stubs.push(Stub::WorkqueuePointer);
                Ok(None)
            }
            External::ReturnThunk => {
                self.expect_bytes(&[0xe9])?;
                Ok(Some(Stop::ReturnThunk))
            }
            External::QueueDelayedWork => {
                self.expect_bytes(&[0xe9])?;
                Ok(Some(Stop::Queue {
                    delay: self.regs[RCX],
                    cpu: self.regs[RDI],
                }))
            }
            External::ColdBranch => {
                self.expect_bytes(&[0x0f, 0x84])?;
                if self.zf {
                    Ok(Some(Stop::ColdPath))
                } else {
                    self.skip(4)?;
                    Ok(None)
                }
            }
        }
    }

    fn decode(&mut self) -> Result<Option<Stop>, Reject> {
        let opcode = self.take()?;
        match opcode {
            0x90 => Ok(None),
            0x66 => self.decode_66(),
            0xf3 => self.decode_endbr(),
            0x0f => self.decode_0f(),
            0x31 => self.decode_xor(),
            0x74 | 0x75 | 0x7f => self.jump_short(opcode),
            0x80 => self.decode_cmpb_imm(),
            0x81 => self.decode_cmpl_imm32(),
            0x83 => self.decode_83(),
            0x85 => self.decode_testl(),
            0x8b => self.decode_movl_load(),
            0xb9 | 0xbf => self.decode_movl_imm(opcode),
            0xc7 => self.decode_movl_store(),
            0xf6 => self.decode_testb(),
            0x48 => self.decode_rex(),
            _ => Err(Reject("unrecognized prefix opcode")),
        }
    }

    fn decode_endbr(&mut self) -> Result<Option<Stop>, Reject> {
        self.expect_bytes(&[0x0f, 0x1e, 0xfa])?;
        Ok(None)
    }

    fn decode_66(&mut self) -> Result<Option<Stop>, Reject> {
        match self.take()? {
            0x90 => Ok(None),
            0x66 => self.decode_nopw(),
            0x81 => self.decode_cmpw_imm(),
            _ => Err(Reject("unrecognized 66-prefix opcode")),
        }
    }

    fn decode_nopw(&mut self) -> Result<Option<Stop>, Reject> {
        self.expect_bytes(&[0x2e, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00])?;
        Ok(None)
    }

    fn decode_0f(&mut self) -> Result<Option<Stop>, Reject> {
        if self.take()? != 0xb6 {
            return Err(Reject("unrecognized 0f opcode"));
        }
        let (reg, addr) = self.mem_operand()?;
        let value = self.read(addr, 1)?;
        self.write_zero_extend(reg, value, 4);
        Ok(None)
    }

    fn decode_xor(&mut self) -> Result<Option<Stop>, Reject> {
        let modrm = self.take()?;
        if modrm != 0xc9 {
            return Err(Reject("unrecognized xor"));
        }
        self.regs[RCX] = 0;
        Ok(None)
    }

    fn decode_cmpb_imm(&mut self) -> Result<Option<Stop>, Reject> {
        let modrm = self.peek()?;
        let reg = (modrm >> 3) & 7;
        if reg == 7 && modrm & 0xc0 == 0xc0 {
            self.take()?;
            let dest = self.regs[usize::from(modrm & 7)] & 0xff;
            let src = u64::from(self.take()?);
            self.flags_sub(dest, src, 8);
            return Ok(None);
        }
        if reg != 7 {
            return Err(Reject("80 opcode is not cmp"));
        }
        let (_reg, addr) = self.mem_operand()?;
        let dest = self.read(addr, 1)?;
        let src = u64::from(self.take()?);
        self.flags_sub(dest, src, 8);
        Ok(None)
    }

    fn decode_cmpl_imm32(&mut self) -> Result<Option<Stop>, Reject> {
        let (_reg, addr) = self.mem_operand_expect(7)?;
        let dest = self.read(addr, 4)?;
        let src = u64::from(self.take_u32()?);
        self.flags_sub(dest, src, 32);
        Ok(None)
    }

    fn decode_cmpw_imm(&mut self) -> Result<Option<Stop>, Reject> {
        let (_reg, addr) = self.mem_operand_expect(7)?;
        let dest = self.read(addr, 2)?;
        let src = u64::from(self.take_u16()?);
        self.flags_sub(dest, src, 16);
        Ok(None)
    }

    fn decode_83(&mut self) -> Result<Option<Stop>, Reject> {
        let modrm = self.peek()?;
        let op = (modrm >> 3) & 7;
        if op == 7 {
            let (_reg, addr) = self.mem_operand()?;
            let dest = self.read(addr, 4)?;
            let src = self.take()? as i8 as u64;
            self.flags_sub(dest, src & 0xffff_ffff, 32);
            return Ok(None);
        }
        Err(Reject("unrecognized 83 opcode outside rex"))
    }

    fn decode_testl(&mut self) -> Result<Option<Stop>, Reject> {
        let modrm = self.take()?;
        if modrm != 0xc0 {
            return Err(Reject("unrecognized testl"));
        }
        let value = self.regs[RAX] & 0xffff_ffff;
        self.flags_test(value, 32);
        Ok(None)
    }

    fn decode_testb(&mut self) -> Result<Option<Stop>, Reject> {
        let (_reg, addr) = self.mem_operand_expect(0)?;
        let dest = self.read(addr, 1)?;
        let src = u64::from(self.take()?);
        self.flags_test(dest & src, 8);
        Ok(None)
    }

    fn decode_movl_load(&mut self) -> Result<Option<Stop>, Reject> {
        let (reg, addr) = self.mem_operand()?;
        let value = self.read(addr, 4)?;
        self.write_zero_extend(reg, value, 4);
        Ok(None)
    }

    fn decode_movl_store(&mut self) -> Result<Option<Stop>, Reject> {
        let (_reg, addr) = self.mem_operand_expect(0)?;
        let value = u64::from(self.take_u32()?);
        self.write(addr, 4, value)?;
        Ok(None)
    }

    fn decode_movl_imm(&mut self, opcode: u8) -> Result<Option<Stop>, Reject> {
        let reg = usize::from(opcode - 0xb8);
        let value = u64::from(self.take_u32()?);
        self.write_zero_extend(reg, value, 4);
        Ok(None)
    }

    fn decode_rex(&mut self) -> Result<Option<Stop>, Reject> {
        match self.take()? {
            0x39 => self.decode_cmpq(),
            0x83 => self.decode_rex_83(),
            0x89 => self.decode_store_reg(),
            0x8b => self.decode_movq_load(),
            0x8d => self.decode_lea(),
            0xba => {
                let value = self.take_u64()?;
                self.regs[RDX] = value;
                Ok(None)
            }
            _ => Err(Reject("unrecognized rex opcode")),
        }
    }

    fn decode_cmpq(&mut self) -> Result<Option<Stop>, Reject> {
        let (reg, addr) = self.mem_operand()?;
        let dest = self.read(addr, 8)?;
        let src = self.regs[reg];
        self.flags_sub(dest, src, 64);
        Ok(None)
    }

    fn decode_rex_83(&mut self) -> Result<Option<Stop>, Reject> {
        let modrm = self.peek()?;
        let op = (modrm >> 3) & 7;
        let rm = usize::from(modrm & 7);
        if modrm & 0xc0 != 0xc0 {
            return Err(Reject("rex 83 memory form is not in this prefix"));
        }
        self.take()?;
        let imm = self.take()? as i8 as u64;
        match op {
            0 => {
                self.regs[rm] = self.regs[rm].wrapping_add(imm);
                Ok(None)
            }
            5 => {
                self.regs[rm] = self.regs[rm].wrapping_sub(imm);
                Ok(None)
            }
            _ => Err(Reject("unrecognized rex 83 operation")),
        }
    }

    fn decode_store_reg(&mut self) -> Result<Option<Stop>, Reject> {
        let modrm = self.peek()?;
        if modrm & 0xc0 == 0xc0 {
            self.take()?;
            let src = usize::from((modrm >> 3) & 7);
            let dest = usize::from(modrm & 7);
            self.regs[dest] = self.regs[src];
            return Ok(None);
        }
        let (reg, addr) = self.mem_operand()?;
        self.write(addr, 8, self.regs[reg])?;
        Ok(None)
    }

    fn decode_movq_load(&mut self) -> Result<Option<Stop>, Reject> {
        let (reg, addr) = self.mem_operand()?;
        let value = self.read(addr, 8)?;
        self.regs[reg] = value;
        Ok(None)
    }

    fn decode_lea(&mut self) -> Result<Option<Stop>, Reject> {
        let (reg, addr) = self.mem_operand()?;
        self.regs[reg] = addr;
        Ok(None)
    }

    fn jump_short(&mut self, opcode: u8) -> Result<Option<Stop>, Reject> {
        let rel = self.take()? as i8 as i64;
        let taken = match opcode {
            0x74 => self.zf,
            0x75 => !self.zf,
            0x7f => !self.zf && self.sf == self.of,
            _ => return Err(Reject("not a short branch")),
        };
        if taken {
            self.rip = self.rip.wrapping_add(rel as u64);
        }
        Ok(None)
    }

    fn mem_operand_expect(&mut self, expect_reg: u8) -> Result<(usize, u64), Reject> {
        let (reg, addr) = self.mem_operand()?;
        if reg != usize::from(expect_reg) {
            return Err(Reject("unexpected modrm register"));
        }
        Ok((reg, addr))
    }

    fn mem_operand(&mut self) -> Result<(usize, u64), Reject> {
        let modrm = self.take()?;
        let mode = modrm >> 6;
        let reg = usize::from((modrm >> 3) & 7);
        let rm = modrm & 7;
        if mode == 3 {
            return Err(Reject("register modrm where memory was required"));
        }
        if rm == 4 {
            let sib = self.take()?;
            if sib != 0x24 || mode != 0 {
                return Err(Reject("only [rsp] sib form is in this prefix"));
            }
            return Ok((reg, self.regs[RSP]));
        }
        if mode == 0 && rm == 5 {
            return Err(Reject("rip-relative memory is an external site"));
        }
        let disp = match mode {
            0 => 0i64,
            1 => i64::from(self.take()? as i8),
            2 => i64::from(self.take_u32()? as i32),
            _ => return Err(Reject("bad addressing mode")),
        };
        Ok((reg, self.regs[usize::from(rm)].wrapping_add(disp as u64)))
    }

    fn flags_sub(&mut self, dest: u64, src: u64, bits: u32) {
        let mask = if bits == 64 {
            u64::MAX
        } else {
            (1u64 << bits) - 1
        };
        let dest = dest & mask;
        let src = src & mask;
        let result = dest.wrapping_sub(src) & mask;
        let sign = 1u64 << (bits - 1);
        self.zf = result == 0;
        self.sf = result & sign != 0;
        self.of = (dest ^ src) & (dest ^ result) & sign != 0;
    }

    fn flags_test(&mut self, value: u64, bits: u32) {
        let mask = (1u64 << bits) - 1;
        let value = value & mask;
        self.zf = value == 0;
        self.sf = value & (1u64 << (bits - 1)) != 0;
        self.of = false;
    }

    fn write_zero_extend(&mut self, reg: usize, value: u64, bytes: u32) {
        let mask = if bytes == 4 {
            0xffff_ffff
        } else {
            (1u64 << (bytes * 8)) - 1
        };
        self.regs[reg] = value & mask;
    }

    fn read(&self, addr: u64, bytes: usize) -> Result<u64, Reject> {
        let raw = self.region(addr, bytes)?;
        let mut out = [0u8; 8];
        out[..bytes].copy_from_slice(raw);
        Ok(u64::from_le_bytes(out))
    }

    fn write(&mut self, addr: u64, bytes: usize, value: u64) -> Result<(), Reject> {
        let encoded = value.to_le_bytes();
        let start = self.region_index(addr, bytes)?;
        match start {
            Region::Device(index) => {
                self.device[index..index + bytes].copy_from_slice(&encoded[..bytes]);
            }
            Region::Stack(index) => {
                self.stack[index..index + bytes].copy_from_slice(&encoded[..bytes]);
            }
            Region::Message(_) => return Err(Reject("prefix tried to write the message")),
        }
        Ok(())
    }

    fn region(&self, addr: u64, bytes: usize) -> Result<&[u8], Reject> {
        match self.region_index(addr, bytes)? {
            Region::Device(index) => Ok(&self.device[index..index + bytes]),
            Region::Message(index) => Ok(&self.message[index..index + bytes]),
            Region::Stack(index) => Ok(&self.stack[index..index + bytes]),
        }
    }

    fn region_index(&self, addr: u64, bytes: usize) -> Result<Region, Reject> {
        if let Some(index) = offset_in(addr, bytes, DEVICE_BASE, self.device.len()) {
            return Ok(Region::Device(index));
        }
        if let Some(index) = offset_in(addr, bytes, MSG_BASE, self.message.len()) {
            return Ok(Region::Message(index));
        }
        if let Some(index) = offset_in(addr, bytes, STACK_BASE, self.stack.len()) {
            return Ok(Region::Stack(index));
        }
        Err(Reject(
            "prefix address is outside the fake device, message, and stack",
        ))
    }

    fn take(&mut self) -> Result<u8, Reject> {
        let index = self
            .rip
            .checked_sub(PREFIX_BASE)
            .ok_or(Reject("rip below the prefix"))?;
        let index = usize::try_from(index).map_err(|_| Reject("rip"))?;
        let byte = *self
            .image
            .bytes
            .get(index)
            .ok_or(Reject("read past the compiled prefix"))?;
        self.rip += 1;
        Ok(byte)
    }

    fn peek(&self) -> Result<u8, Reject> {
        let index = usize::try_from(self.rip - PREFIX_BASE).map_err(|_| Reject("rip"))?;
        self.image
            .bytes
            .get(index)
            .copied()
            .ok_or(Reject("read past the compiled prefix"))
    }

    fn take_u16(&mut self) -> Result<u16, Reject> {
        Ok(u16::from_le_bytes([self.take()?, self.take()?]))
    }

    fn take_u32(&mut self) -> Result<u32, Reject> {
        Ok(u32::from_le_bytes([
            self.take()?,
            self.take()?,
            self.take()?,
            self.take()?,
        ]))
    }

    fn take_u64(&mut self) -> Result<u64, Reject> {
        let mut raw = [0u8; 8];
        for slot in &mut raw {
            *slot = self.take()?;
        }
        Ok(u64::from_le_bytes(raw))
    }

    fn skip(&mut self, count: u64) -> Result<(), Reject> {
        for _ in 0..count {
            self.take()?;
        }
        Ok(())
    }

    fn expect_bytes(&mut self, bytes: &[u8]) -> Result<(), Reject> {
        for expected in bytes {
            if self.take()? != *expected {
                return Err(Reject("external site opcode does not match the object"));
            }
        }
        Ok(())
    }
}

enum Region {
    Device(usize),
    Message(usize),
    Stack(usize),
}

fn offset_in(addr: u64, bytes: usize, base: u64, len: usize) -> Option<usize> {
    let index = usize::try_from(addr.checked_sub(base)?).ok()?;
    let end = index.checked_add(bytes)?;
    if end <= len { Some(index) } else { None }
}

fn run_image(image: &Image, message: &[u8], startup: i32) -> Result<Outcome, Reject> {
    Machine::new(image, message, startup)?.run()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(bytes))
}

/// Synthetic SysEx input. This buffer is not the Linux instruction array.
fn identity_packet(family_low: u8, family_high: u8, model_low: u8, model_high: u8) -> [u8; 16] {
    let mut packet = [0u8; 16];
    packet[0] = 0xf0;
    packet[1..9].copy_from_slice(&linux_transport::IDENTITY_PREFIX);
    packet[8] = family_low;
    packet[9] = family_high;
    packet[10] = model_low;
    packet[11] = model_high;
    packet
}

fn expect_identity(image_is_patched: bool, packet: &[u8], progress: i32) -> Result<(), Reject> {
    let image = if image_is_patched {
        &PATCHED
    } else {
        &UNPATCHED
    };
    let outcome = run_image(image, packet, progress)?;
    let matched = if image_is_patched {
        linux_transport::matches_patched_startup_identity(packet)
    } else {
        linux_transport::matches_startup_identity(packet)
    };
    let expected_state = if image_is_patched {
        linux_transport::startup_state_after_patched_identity(packet, progress)
    } else {
        linux_transport::startup_state_after_identity(packet, progress)
    };
    if outcome.startup != expected_state {
        return Err(Reject("compiled state store disagreed with the Rust gate"));
    }
    let queues = matched && progress < STARTUP_ACTIVATE;
    match outcome.stop {
        Stop::Queue { delay, cpu } if queues => {
            if delay != 0x64 || cpu != QUEUE_CPU {
                return Err(Reject(
                    "identity queue arguments were not delay 100 and cpu 0x2000",
                ));
            }
            if outcome.stubs
                != [
                    Stub::Fentry,
                    Stub::CancelDelayedWork,
                    Stub::WorkqueuePointer,
                ]
            {
                return Err(Reject("identity schedule stub sequence changed"));
            }
        }
        Stop::ReturnThunk if !queues && packet.first() != Some(&0xff) => {
            if outcome.stubs != [Stub::Fentry] || (outcome.startup != progress && !matched) {
                return Err(Reject(
                    "rejected identity packet changed progress or called out",
                ));
            }
            if outcome.startup != expected_state {
                return Err(Reject("return path lost the progress value"));
            }
        }
        _ => return Err(Reject("compiled prefix stopped in an unexpected way")),
    }
    if linux_transport::matches_init_done(packet) {
        return Err(Reject("identity fixture collided with init-done"));
    }
    Ok(())
}

fn check_prefixes() -> Result<(), Reject> {
    if usize::try_from(STARTUP_STATE_OFFSET) != Ok(0x2128) {
        return Err(Reject("startup offset moved"));
    }
    if sha256_hex(&UNPATCHED_PREFIX) != UNPATCHED_PREFIX_SHA256
        || sha256_hex(&PATCHED_PREFIX) != PATCHED_PREFIX_SHA256
    {
        return Err(Reject(
            "compiled prefix bytes no longer match the recorded hashes",
        ));
    }
    if !PREFIX_NOTICE.contains("SPDX-License-Identifier: GPL-2.0-only")
        || !PREFIX_NOTICE.contains(linux_transport::UPSTREAM_VARIAX_C_COMMIT)
        || !PREFIX_NOTICE.contains("Copyright (C) 2004-2010 Markus Grabner")
        || !PREFIX_NOTICE.contains("[0x160, 0x240)")
        || !PREFIX_NOTICE.contains("not a runtime dependency")
        || !GPL_TEXT.contains("GNU GENERAL PUBLIC LICENSE")
        || !GPL_TEXT.contains("Version 2, June 1991")
    {
        return Err(Reject(
            "kernel reference-byte notice lost its GPL-2.0-only attribution",
        ));
    }
    let boundaries = [i32::MIN, -1, 0, 1, 2, i32::MAX];
    for patched in [false, true] {
        for progress in boundaries {
            for model in 0u8..=255 {
                expect_identity(patched, &identity_packet(0x07, 0x00, model, 0x00), progress)?;
            }
        }
        for model in [0x00u8, 0x04] {
            for value in 0u8..=255 {
                expect_identity(patched, &identity_packet(value, 0x00, model, 0x00), 0)?;
                expect_identity(patched, &identity_packet(0x07, value, model, 0x00), 0)?;
                expect_identity(patched, &identity_packet(0x07, 0x00, model, value), 0)?;
            }
        }
    }
    let mut init_done = [0u8; 16];
    init_done[..INIT_DONE_PREFIX.len()].copy_from_slice(&INIT_DONE_PREFIX);
    for image in [&UNPATCHED, &PATCHED] {
        for progress in boundaries {
            let outcome = run_image(image, &init_done, progress)?;
            if outcome.startup != progress {
                return Err(Reject("init-done stored identity progress"));
            }
            let schedules = linux_transport::init_done_schedules_startup(&init_done, progress);
            match outcome.stop {
                Stop::Queue { delay, cpu } if schedules => {
                    if delay != 0 || cpu != QUEUE_CPU {
                        return Err(Reject("init-done queue did not use delay 0"));
                    }
                    if outcome.stubs
                        != [
                            Stub::Fentry,
                            Stub::CancelDelayedWork,
                            Stub::WorkqueuePointer,
                        ]
                    {
                        return Err(Reject("init-done stub sequence changed"));
                    }
                }
                Stop::ReturnThunk if !schedules => {
                    if outcome.stubs != [Stub::Fentry] {
                        return Err(Reject("skipped init-done still called the scheduler"));
                    }
                }
                _ => return Err(Reject("init-done stop did not follow the progress gate")),
            }
        }
        let other = run_image(image, &[0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0)?;
        if other.startup != 0 || other.stop != Stop::ReturnThunk || other.stubs != [Stub::Fentry] {
            return Err(Reject("non-sysex byte entered the identity compare"));
        }
        let cold = run_image(image, &[0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0)?;
        if cold.startup != 0 || cold.stop != Stop::ColdPath || cold.stubs != [Stub::Fentry] {
            return Err(Reject("cold path was interpreted or changed progress"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emulates_the_compiled_prefix_without_kernel_execution() {
        assert_eq!(
            check_prefixes(),
            Ok(()),
            "compiled identity prefix disagreed with its state gate or followed a kernel call"
        );
    }
}
