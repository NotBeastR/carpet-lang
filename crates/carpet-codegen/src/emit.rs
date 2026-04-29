use std::collections::HashMap;

use carpet_ir::ssa::{
    BasicBlock, BuiltinFunc, Constant, Function, IROp, Instruction, Module, StringId, VReg,
    ValueType,
};

use crate::output::{CodegenOutput, RelocKind, Relocation};
use crate::target::Target;
use crate::x86_64::X86_64Encoder;

const BUFFER_SIZE: u64 = 131072;
const BSS_TOTAL: u64 = BUFFER_SIZE + 8;

struct VRegSlot {
    rbp_offset: i32,
    value_type: ValueType,
}

struct StringInfo {
    rodata_offset: usize,
    len: usize,
}

pub struct Emitter {
    enc: X86_64Encoder,
    rodata: Vec<u8>,
    relocations: Vec<Relocation>,
    target: Target,
    vreg_slots: HashMap<VReg, VRegSlot>,
    var_slots: HashMap<String, VRegSlot>,
    string_info: HashMap<StringId, StringInfo>,
    stack_size: u32,
    sign_mask_rodata_offset: usize,
    ten_const_rodata_offset: usize,
    runtime_flush_offset: Option<usize>,
    runtime_say_number_offset: Option<usize>,
    runtime_say_string_offset: Option<usize>,
    runtime_buffer_write_offset: Option<usize>,
    runtime_int_to_str_offset: Option<usize>,
}

impl Emitter {
    pub fn new(target: Target) -> Self {
        Self {
            enc: X86_64Encoder::new(),
            rodata: Vec::new(),
            relocations: Vec::new(),
            target,
            vreg_slots: HashMap::new(),
            var_slots: HashMap::new(),
            string_info: HashMap::new(),
            stack_size: 0,
            sign_mask_rodata_offset: 0,
            ten_const_rodata_offset: 0,
            runtime_flush_offset: None,
            runtime_say_number_offset: None,
            runtime_say_string_offset: None,
            runtime_buffer_write_offset: None,
            runtime_int_to_str_offset: None,
        }
    }

    pub fn emit(mut self, module: &Module) -> CodegenOutput {
        self.emit_rodata_constants(module);
        self.emit_rodata_specials();

        self.emit_runtime_flush();
        self.emit_runtime_buffer_write();
        self.emit_runtime_int_to_str();
        self.emit_runtime_say_number();
        self.emit_runtime_say_string();

        let entry_offset = self.enc.pos();
        self.emit_entry(module);

        CodegenOutput {
            text: self.enc.code,
            rodata: self.rodata,
            bss_size: BSS_TOTAL,
            entry_offset: entry_offset as u64,
            relocations: self.relocations,
            target: self.target,
            sign_mask_rodata_offset: self.sign_mask_rodata_offset,
            ten_const_rodata_offset: self.ten_const_rodata_offset,
        }
    }

    fn emit_rodata_constants(&mut self, module: &Module) {
        for sc in &module.strings {
            let offset = self.rodata.len();
            self.rodata.extend_from_slice(sc.value.as_bytes());
            self.string_info.insert(
                sc.id,
                StringInfo {
                    rodata_offset: offset,
                    len: sc.value.len(),
                },
            );
        }
    }

    fn emit_rodata_specials(&mut self) {
        while self.rodata.len() % 16 != 0 {
            self.rodata.push(0);
        }
        self.sign_mask_rodata_offset = self.rodata.len();
        self.rodata
            .extend_from_slice(&0x8000000000000000u64.to_le_bytes());
        self.rodata
            .extend_from_slice(&0x8000000000000000u64.to_le_bytes());

        self.ten_const_rodata_offset = self.rodata.len();
        self.rodata.extend_from_slice(&10.0f64.to_le_bytes());
    }

    fn alloc_stack_slot(&mut self, ty: ValueType) -> i32 {
        let size: u32 = match ty {
            ValueType::Number => 8,
            ValueType::String => 16,
        };
        self.stack_size += size;
        -(self.stack_size as i32)
    }

    fn emit_entry(&mut self, module: &Module) {
        self.allocate_slots(module);

        self.enc.push_rbp();
        self.enc.mov_rbp_rsp();

        // Emit sub rsp with placeholder - will patch after all code is emitted
        // since emit_store_var can allocate additional stack slots
        self.enc.sub_rsp_imm32(0);
        let stack_patch_offset = self.enc.pos() - 4;

        for func in &module.functions {
            self.emit_function(func, module);
        }

        // Patch the stack allocation with final size (vregs + variables)
        let stack_alloc = (self.stack_size + 15) & !15;
        self.enc
            .patch_i32_le(stack_patch_offset, stack_alloc as i32);

        let flush_off = self.runtime_flush_offset.unwrap();
        self.emit_call_internal(flush_off);

        self.enc.mov_rax_imm32(self.target.sys_exit() as u32);
        self.enc.mov_rdi_imm32(0);
        self.enc.syscall();
    }

    fn emit_call_internal(&mut self, target_offset: usize) {
        let call_site = self.enc.pos();
        self.enc.call_rel32(0);
        let rel = target_offset as i32 - (call_site as i32 + 5);
        self.enc.patch_i32_le(call_site + 1, rel);
    }

    fn allocate_slots(&mut self, module: &Module) {
        for (vreg_idx, &ty) in module.vreg_types.iter().enumerate() {
            let offset = self.alloc_stack_slot(ty);
            self.vreg_slots.insert(
                vreg_idx as u32,
                VRegSlot {
                    rbp_offset: offset,
                    value_type: ty,
                },
            );
        }
    }

    fn emit_function(&mut self, func: &Function, module: &Module) {
        for block in &func.blocks {
            self.emit_block(block, module);
        }
    }

    fn emit_block(&mut self, block: &BasicBlock, module: &Module) {
        for instr in &block.instructions {
            self.emit_instruction(instr, module);
        }
    }

    fn emit_instruction(&mut self, instr: &Instruction, _module: &Module) {
        match instr {
            Instruction::Const { dest, value } => {
                self.emit_const(*dest, value);
            }
            Instruction::BinOp {
                dest,
                op,
                left,
                right,
            } => {
                self.emit_binop(*dest, *op, *left, *right);
            }
            Instruction::Neg { dest, src } => {
                self.emit_neg(*dest, *src);
            }
            Instruction::Call { func, args } => {
                self.emit_builtin_call(func, args);
            }
            Instruction::StoreVar { name, src } => {
                self.emit_store_var(name, *src);
            }
            Instruction::LoadVar { dest, name } => {
                self.emit_load_var(*dest, name);
            }
        }
    }

    fn emit_const(&mut self, dest: VReg, value: &Constant) {
        let slot_off = self.vreg_slots[&dest].rbp_offset;
        match value {
            Constant::Number(n) => {
                let bits = n.to_bits();
                self.enc.mov_rax_imm64(bits);
                self.enc.mov_rbp_disp32_rax(slot_off);
            }
            Constant::String(sid) => {
                let info = &self.string_info[sid];
                let rodata_off = info.rodata_offset;
                let len = info.len;
                let reloc_offset = self.enc.lea_rax_rip_rel32();
                self.relocations.push(Relocation {
                    offset: reloc_offset,
                    kind: RelocKind::RodataOffset(rodata_off),
                });
                self.enc.mov_rbp_disp32_rax(slot_off);
                self.enc.mov_rax_imm64(len as u64);
                self.enc.mov_rbp_disp32_rax(slot_off + 8);
            }
        }
    }

    fn emit_binop(&mut self, dest: VReg, op: IROp, left: VReg, right: VReg) {
        let left_off = self.vreg_slots[&left].rbp_offset;
        let right_off = self.vreg_slots[&right].rbp_offset;
        let dest_off = self.vreg_slots[&dest].rbp_offset;

        self.enc.movsd_xmm0_rbp_disp32(left_off);
        self.enc.movsd_xmm1_rbp_disp32(right_off);

        match op {
            IROp::Add => self.enc.addsd_xmm0_xmm1(),
            IROp::Sub => self.enc.subsd_xmm0_xmm1(),
            IROp::Mul => self.enc.mulsd_xmm0_xmm1(),
            IROp::Div => self.enc.divsd_xmm0_xmm1(),
            IROp::Mod => {
                self.enc.movapd_xmm1_xmm0();
                self.enc.movsd_xmm0_rbp_disp32(left_off);
                self.enc.movsd_xmm1_rbp_disp32(right_off);
                self.enc.movsd_rbp_disp32_xmm0(dest_off);
                self.enc.divsd_xmm0_xmm1();
                self.enc.cvttsd2si_rax_xmm0();
                self.enc.cvtsi2sd_xmm0_rax();
                self.enc.movsd_xmm1_rbp_disp32(right_off);
                self.enc.mulsd_xmm0_xmm1();
                self.enc.movapd_xmm1_xmm0();
                self.enc.movsd_xmm0_rbp_disp32(dest_off);
                self.enc.subsd_xmm0_xmm1();
            }
        }

        self.enc.movsd_rbp_disp32_xmm0(dest_off);
    }

    fn emit_neg(&mut self, dest: VReg, src: VReg) {
        let src_off = self.vreg_slots[&src].rbp_offset;
        let dest_off = self.vreg_slots[&dest].rbp_offset;

        self.enc.movsd_xmm0_rbp_disp32(src_off);
        let reloc_offset = self.enc.xorpd_xmm0_rip_rel32();
        self.relocations.push(Relocation {
            offset: reloc_offset,
            kind: RelocKind::SignMaskAddr,
        });
        self.enc.movsd_rbp_disp32_xmm0(dest_off);
    }

    fn emit_builtin_call(&mut self, func: &BuiltinFunc, args: &[VReg]) {
        match func {
            BuiltinFunc::SayNumber => {
                let arg = args[0];
                let arg_off = self.vreg_slots[&arg].rbp_offset;
                self.enc.movsd_xmm0_rbp_disp32(arg_off);
                let target = self.runtime_say_number_offset.unwrap();
                self.emit_call_internal(target);
            }
            BuiltinFunc::SayString => {
                let arg = args[0];
                let arg_off = self.vreg_slots[&arg].rbp_offset;
                self.enc.mov_rdi_rbp_disp32(arg_off);
                self.enc.mov_rsi_rbp_disp32(arg_off + 8);
                let target = self.runtime_say_string_offset.unwrap();
                self.emit_call_internal(target);
            }
        }
    }

    fn emit_store_var(&mut self, name: &str, src: VReg) {
        let src_ty = self.vreg_slots[&src].value_type;
        let src_off = self.vreg_slots[&src].rbp_offset;

        if !self.var_slots.contains_key(name) {
            let offset = self.alloc_stack_slot(src_ty);
            self.var_slots.insert(
                name.to_string(),
                VRegSlot {
                    rbp_offset: offset,
                    value_type: src_ty,
                },
            );
        }

        let var_off = self.var_slots[name].rbp_offset;

        match src_ty {
            ValueType::Number => {
                self.enc.movsd_xmm0_rbp_disp32(src_off);
                self.enc.movsd_rbp_disp32_xmm0(var_off);
            }
            ValueType::String => {
                self.enc.mov_rax_rbp_disp32(src_off);
                self.enc.mov_rbp_disp32_rax(var_off);
                self.enc.mov_rax_rbp_disp32(src_off + 8);
                self.enc.mov_rbp_disp32_rax(var_off + 8);
            }
        }
    }

    fn emit_load_var(&mut self, dest: VReg, name: &str) {
        let var_ty = self.var_slots[name].value_type;
        let var_off = self.var_slots[name].rbp_offset;
        let dest_off = self.vreg_slots[&dest].rbp_offset;

        match var_ty {
            ValueType::Number => {
                self.enc.movsd_xmm0_rbp_disp32(var_off);
                self.enc.movsd_rbp_disp32_xmm0(dest_off);
            }
            ValueType::String => {
                self.enc.mov_rax_rbp_disp32(var_off);
                self.enc.mov_rbp_disp32_rax(dest_off);
                self.enc.mov_rax_rbp_disp32(var_off + 8);
                self.enc.mov_rbp_disp32_rax(dest_off + 8);
            }
        }
    }

    fn emit_runtime_flush(&mut self) {
        self.runtime_flush_offset = Some(self.enc.pos());

        self.enc.push_rbp();
        self.enc.mov_rbp_rsp();

        let reloc1 = self.enc.mov_rax_rip_rel32();
        self.relocations.push(Relocation {
            offset: reloc1,
            kind: RelocKind::BssBufferPosAddr,
        });

        self.enc.test_rax_rax();
        let skip_jmp = self.enc.je_rel32();

        self.enc.mov_rdx_rax();
        self.enc.mov_rax_imm32(self.target.sys_write() as u32);
        self.enc.mov_rdi_imm32(1);
        let reloc2 = self.enc.lea_rsi_rip_rel32();
        self.relocations.push(Relocation {
            offset: reloc2,
            kind: RelocKind::BssBufferAddr,
        });
        self.enc.syscall();

        self.enc.xor_eax_eax();
        let reloc3 = self.enc.mov_rip_rel32_rax();
        self.relocations.push(Relocation {
            offset: reloc3,
            kind: RelocKind::BssBufferPosAddr,
        });

        self.enc.patch_rel32(skip_jmp);
        self.enc.pop_rbp();
        self.enc.ret();
    }

    fn emit_runtime_buffer_write(&mut self) {
        self.runtime_buffer_write_offset = Some(self.enc.pos());

        // (rdi = src_ptr, rsi = src_len)
        self.enc.push_rbp();
        self.enc.mov_rbp_rsp();
        self.enc.sub_rsp_imm32(32);

        // [rbp-8] = src_ptr, [rbp-16] = src_len
        self.enc.code.extend_from_slice(&[0x48, 0x89, 0x7D, 0xF8]); // mov [rbp-8], rdi
        self.enc.code.extend_from_slice(&[0x48, 0x89, 0x75, 0xF0]); // mov [rbp-16], rsi

        // Load buffer_pos
        let reloc1 = self.enc.mov_rax_rip_rel32();
        self.relocations.push(Relocation {
            offset: reloc1,
            kind: RelocKind::BssBufferPosAddr,
        });
        // [rbp-24] = buffer_pos
        self.enc.code.extend_from_slice(&[0x48, 0x89, 0x45, 0xE8]); // mov [rbp-24], rax

        // Check pos + len >= BUFFER_SIZE
        self.enc.mov_rsi_rbp_disp32(-16);
        self.enc.add_rax_rsi();
        self.enc.mov_rdx_imm64(BUFFER_SIZE);
        self.enc.code.extend_from_slice(&[0x48, 0x39, 0xD0]); // cmp rax, rdx
        self.enc.jb_rel8(0);
        let no_flush_patch = self.enc.pos();

        // Flush
        let flush_off = self.runtime_flush_offset.unwrap();
        self.emit_call_internal(flush_off);
        // Reset buffer_pos to 0
        self.enc
            .code
            .extend_from_slice(&[0x48, 0xC7, 0x45, 0xE8, 0x00, 0x00, 0x00, 0x00]);

        let after_flush = self.enc.pos();
        self.enc.code[no_flush_patch - 1] = (after_flush - no_flush_patch) as u8;

        // rdi = buffer + buffer_pos
        let reloc2 = self.enc.lea_rdi_rip_rel32();
        self.relocations.push(Relocation {
            offset: reloc2,
            kind: RelocKind::BssBufferAddr,
        });
        self.enc.mov_rax_rbp_disp32(-24);
        self.enc.add_rdi_rax();

        // rsi = src_ptr
        self.enc.mov_rsi_rbp_disp32(-8);
        // rcx = src_len
        self.enc.code.extend_from_slice(&[0x48, 0x8B, 0x4D, 0xF0]); // mov rcx, [rbp-16]
                                                                    // rep movsb
        self.enc.rep_movsb();

        // Update buffer_pos
        self.enc.mov_rax_rbp_disp32(-24);
        self.enc.code.extend_from_slice(&[0x48, 0x03, 0x45, 0xF0]); // add rax, [rbp-16]
        let reloc3 = self.enc.mov_rip_rel32_rax();
        self.relocations.push(Relocation {
            offset: reloc3,
            kind: RelocKind::BssBufferPosAddr,
        });

        self.enc.add_rsp_imm32(32);
        self.enc.pop_rbp();
        self.enc.ret();
    }

    fn emit_runtime_int_to_str(&mut self) {
        self.runtime_int_to_str_offset = Some(self.enc.pos());

        // (rdi = buf_ptr, rsi = unsigned_int_value) -> rax = chars_written
        self.enc.push_rbp();
        self.enc.mov_rbp_rsp();
        self.enc.sub_rsp_imm32(32);

        // [rbp-8] = buf_ptr
        self.enc.code.extend_from_slice(&[0x48, 0x89, 0x7D, 0xF8]); // mov [rbp-8], rdi

        // rax = value
        self.enc.code.extend_from_slice(&[0x48, 0x89, 0xF0]); // mov rax, rsi

        // If 0, write '0' and return 1
        self.enc.test_rax_rax();
        let not_zero = self.enc.jne_rel32();
        self.enc.code.extend_from_slice(&[0xC6, 0x07, 0x30]); // mov byte [rdi], '0'
        self.enc.mov_rax_imm64(1);
        self.enc.add_rsp_imm32(32);
        self.enc.pop_rbp();
        self.enc.ret();

        self.enc.patch_rel32(not_zero);

        // r8 = 10
        self.enc.mov_r8_imm64(10);
        // rcx = digit count
        self.enc.code.extend_from_slice(&[0x48, 0x31, 0xC9]); // xor rcx, rcx

        let loop_start = self.enc.pos();
        self.enc.xor_rdx_rdx();
        self.enc.div_r8();
        self.enc.code.extend_from_slice(&[0x80, 0xC2, 0x30]); // add dl, '0'
        self.enc.push_rdx();
        self.enc.code.extend_from_slice(&[0x48, 0xFF, 0xC1]); // inc rcx
        self.enc.test_rax_rax();
        let loop_back = self.enc.jne_rel32();
        let rel = loop_start as i32 - self.enc.pos() as i32;
        self.enc.patch_i32_le(loop_back, rel);

        // [rbp-16] = digit count
        self.enc.code.extend_from_slice(&[0x48, 0x89, 0x4D, 0xF0]); // mov [rbp-16], rcx

        // Pop digits into buffer
        self.enc.mov_rdi_rbp_disp32(-8);

        let pop_loop = self.enc.pos();
        self.enc.pop_rax();
        self.enc.mov_rdi_deref_al();
        self.enc.inc_rdi();
        self.enc.dec_rcx();
        self.enc.test_rcx_rcx();
        let pop_back = self.enc.jne_rel32();
        let rel = pop_loop as i32 - self.enc.pos() as i32;
        self.enc.patch_i32_le(pop_back, rel);

        self.enc.mov_rax_rbp_disp32(-16);
        self.enc.add_rsp_imm32(32);
        self.enc.pop_rbp();
        self.enc.ret();
    }

    fn emit_runtime_say_number(&mut self) {
        self.runtime_say_number_offset = Some(self.enc.pos());

        // (xmm0 = f64 value)
        self.enc.push_rbp();
        self.enc.mov_rbp_rsp();
        self.enc.sub_rsp_imm32(112);

        // [rbp-8] = original value (f64)
        self.enc.movsd_rbp_disp32_xmm0(-8);

        // Local buffer at [rbp-80] to [rbp-16], 64 bytes for number string
        // [rbp-88] = write cursor offset (from [rbp-80])
        // [rbp-96] = original abs value for frac computation

        // Check NaN
        self.enc.code.extend_from_slice(&[0x66, 0x0F, 0x2E, 0xC0]); // ucomisd xmm0, xmm0
        let nan_jmp = self.enc.jp_rel32();

        // Check negative
        self.enc.xorpd_xmm1_xmm1();
        self.enc.ucomisd_xmm0_xmm1();
        let not_neg = self.enc.jae_rel32();

        // Write '-'
        self.enc.code.extend_from_slice(&[0xC6, 0x45, 0xB0, 0x2D]); // mov byte [rbp-80], '-'
                                                                    // cursor = 1
        self.enc
            .code
            .extend_from_slice(&[0x48, 0xC7, 0x45, 0xA8, 0x01, 0x00, 0x00, 0x00]); // [rbp-88] = 1

        // Negate
        let reloc_neg = self.enc.xorpd_xmm0_rip_rel32();
        self.relocations.push(Relocation {
            offset: reloc_neg,
            kind: RelocKind::SignMaskAddr,
        });

        let after_sign = self.enc.jmp_rel32();

        self.enc.patch_rel32(not_neg);
        // cursor = 0
        self.enc
            .code
            .extend_from_slice(&[0x48, 0xC7, 0x45, 0xA8, 0x00, 0x00, 0x00, 0x00]); // [rbp-88] = 0

        self.enc.patch_rel32(after_sign);

        // [rbp-96] = abs value
        self.enc.movsd_rbp_disp32_xmm0(-96);

        // Integer part
        self.enc.cvttsd2si_rax_xmm0();
        self.enc.code.extend_from_slice(&[0x48, 0x89, 0x45, 0x98]); // [rbp-104] = int_part

        // Fractional part = xmm0 - (double)int_part
        self.enc.cvtsi2sd_xmm1_rax();
        self.enc.subsd_xmm0_xmm1();
        self.enc.movsd_rbp_disp32_xmm0(-112); // [rbp-112] = frac

        // Convert integer part: rdi = &buffer[cursor], rsi = int_part
        self.enc.code.extend_from_slice(&[0x48, 0x8D, 0x7D, 0xB0]); // lea rdi, [rbp-80]
        self.enc.mov_rax_rbp_disp32(-88); // cursor
        self.enc.add_rdi_rax();
        self.enc.code.extend_from_slice(&[0x48, 0x8B, 0x75, 0x98]); // mov rsi, [rbp-104]
        let int_to_str = self.runtime_int_to_str_offset.unwrap();
        self.emit_call_internal(int_to_str);

        // cursor += chars_written
        self.enc.code.extend_from_slice(&[0x48, 0x01, 0x45, 0xA8]); // add [rbp-88], rax

        // Check fractional part
        self.enc.movsd_xmm0_rbp_disp32(-112);
        self.enc.xorpd_xmm1_xmm1();
        self.enc.ucomisd_xmm0_xmm1();
        let no_frac = self.enc.je_rel32();

        // Write '.'
        self.enc.code.extend_from_slice(&[0x48, 0x8D, 0x7D, 0xB0]); // lea rdi, [rbp-80]
        self.enc.mov_rax_rbp_disp32(-88);
        self.enc.add_rdi_rax();
        self.enc.code.extend_from_slice(&[0xC6, 0x07, 0x2E]); // mov byte [rdi], '.'
                                                              // cursor++
        self.enc.code.extend_from_slice(&[0x48, 0xFF, 0x45, 0xA8]); // inc qword [rbp-88]

        // Frac digit loop (max 15 digits)
        self.enc.movsd_xmm0_rbp_disp32(-112);
        self.enc.mov_rcx_imm64(15);

        // Save frac digit start cursor for trailing zero strip
        self.enc.mov_rax_rbp_disp32(-88);
        self.enc.code.extend_from_slice(&[0x49, 0x89, 0xC0]); // mov r8, rax  (frac_start)

        let frac_loop = self.enc.pos();

        // frac *= 10
        let reloc_ten = self.enc.mulsd_xmm0_rip_rel32();
        self.relocations.push(Relocation {
            offset: reloc_ten,
            kind: RelocKind::TenConstAddr,
        });

        // digit = (int)frac
        self.enc.push_rcx();
        self.enc.cvttsd2si_rax_xmm0();
        self.enc.cvtsi2sd_xmm1_rax();
        self.enc.subsd_xmm0_xmm1();

        // Store digit
        self.enc.add_al_imm8(0x30);
        // buffer[cursor] = digit + '0'
        self.enc.push_rax(); // save digit
        self.enc.mov_rax_rbp_disp32(-88); // cursor
        self.enc.code.extend_from_slice(&[0x48, 0x8D, 0x7D, 0xB0]); // lea rdi, [rbp-80]
        self.enc.add_rdi_rax();
        self.enc.pop_rax();
        self.enc.mov_rdi_deref_al();
        // cursor++
        self.enc.code.extend_from_slice(&[0x48, 0xFF, 0x45, 0xA8]);

        self.enc.pop_rcx();
        self.enc.dec_rcx();

        // Check frac == 0
        self.enc.push_rcx();
        self.enc.xorpd_xmm1_xmm1();
        self.enc.ucomisd_xmm0_xmm1();
        self.enc.pop_rcx();
        let frac_zero = self.enc.je_rel32();

        self.enc.test_rcx_rcx();
        let frac_cont = self.enc.jne_rel32();
        let rel = frac_loop as i32 - self.enc.pos() as i32;
        self.enc.patch_i32_le(frac_cont, rel);

        self.enc.patch_rel32(frac_zero);

        // Strip trailing zeros
        self.enc.mov_rax_rbp_disp32(-88); // cursor (end)
        let strip_loop = self.enc.pos();
        self.enc.sub_rax_imm8(1);
        // Check buffer[rax]
        self.enc.code.extend_from_slice(&[0x48, 0x8D, 0x7D, 0xB0]); // lea rdi, [rbp-80]
        self.enc.push_rax();
        self.enc.add_rdi_rax();
        self.enc.mov_al_rdi_deref();
        self.enc.cmp_al_imm8(0x30); // '0'
        self.enc.pop_rax();
        self.enc.push_rax();
        let not_zero = self.enc.jne_rel32();
        self.enc.pop_rax();

        // Also check we haven't gone past the decimal point
        self.enc.code.extend_from_slice(&[0x4C, 0x39, 0xC0]); // cmp rax, r8
        let past_frac_start = self.enc.jae_rel32();
        let rel = strip_loop as i32 - self.enc.pos() as i32;
        self.enc.patch_i32_le(past_frac_start, rel);

        // Went past, restore
        self.enc.add_rax_imm8(1);
        let strip_end = self.enc.jmp_rel32();

        self.enc.patch_rel32(not_zero);
        self.enc.pop_rax();
        self.enc.add_rax_imm8(1);

        self.enc.patch_rel32(strip_end);

        // Update cursor
        self.enc.mov_rbp_disp32_rax(-88);

        let after_frac = self.enc.jmp_rel32();
        self.enc.patch_rel32(no_frac);
        self.enc.patch_rel32(after_frac);

        // Write to output buffer
        self.enc.code.extend_from_slice(&[0x48, 0x8D, 0x7D, 0xB0]); // lea rdi, [rbp-80]  (src ptr)
        self.enc.mov_rsi_rbp_disp32(-88); // rsi = length (cursor value)

        let bw = self.runtime_buffer_write_offset.unwrap();
        self.emit_call_internal(bw);

        // Jump past NaN handler to newline
        let to_newline = self.enc.jmp_rel32();

        // NaN handler
        self.enc.patch_rel32(nan_jmp);
        self.enc.code.extend_from_slice(&[0xC6, 0x45, 0xB0, 0x4E]); // 'N'
        self.enc.code.extend_from_slice(&[0xC6, 0x45, 0xB1, 0x61]); // 'a'
        self.enc.code.extend_from_slice(&[0xC6, 0x45, 0xB2, 0x4E]); // 'N'
        self.enc.code.extend_from_slice(&[0x48, 0x8D, 0x7D, 0xB0]);
        self.enc.mov_rsi_imm64(3);
        self.emit_call_internal(bw);

        self.enc.patch_rel32(to_newline);

        // Write newline
        self.enc.code.extend_from_slice(&[0xC6, 0x45, 0xB0, 0x0A]); // '\n'
        self.enc.code.extend_from_slice(&[0x48, 0x8D, 0x7D, 0xB0]);
        self.enc.mov_rsi_imm64(1);
        self.emit_call_internal(bw);

        self.enc.add_rsp_imm32(112);
        self.enc.pop_rbp();
        self.enc.ret();
    }

    fn emit_runtime_say_string(&mut self) {
        self.runtime_say_string_offset = Some(self.enc.pos());

        // (rdi = str_ptr, rsi = str_len)
        self.enc.push_rbp();
        self.enc.mov_rbp_rsp();
        self.enc.sub_rsp_imm32(16);

        // Save for newline write
        self.enc.code.extend_from_slice(&[0x48, 0x89, 0x7D, 0xF8]); // [rbp-8] = rdi
        self.enc.code.extend_from_slice(&[0x48, 0x89, 0x75, 0xF0]); // [rbp-16] = rsi

        let bw = self.runtime_buffer_write_offset.unwrap();
        self.emit_call_internal(bw);

        // Write newline
        self.enc.code.extend_from_slice(&[0xC6, 0x45, 0xF8, 0x0A]); // mov byte [rbp-8], '\n'
        self.enc.code.extend_from_slice(&[0x48, 0x8D, 0x7D, 0xF8]); // lea rdi, [rbp-8]
        self.enc.mov_rsi_imm64(1);
        self.emit_call_internal(bw);

        self.enc.add_rsp_imm32(16);
        self.enc.pop_rbp();
        self.enc.ret();
    }
}
