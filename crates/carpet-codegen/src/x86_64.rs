pub struct X86_64Encoder {
    pub code: Vec<u8>,
}

impl Default for X86_64Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl X86_64Encoder {
    pub fn new() -> Self {
        Self { code: Vec::new() }
    }

    pub fn pos(&self) -> usize {
        self.code.len()
    }

    pub fn push_u8(&mut self, b: u8) {
        self.code.push(b);
    }

    pub fn push_u32_le(&mut self, v: u32) {
        self.code.extend_from_slice(&v.to_le_bytes());
    }

    pub fn push_u64_le(&mut self, v: u64) {
        self.code.extend_from_slice(&v.to_le_bytes());
    }

    pub fn push_i32_le(&mut self, v: i32) {
        self.code.extend_from_slice(&v.to_le_bytes());
    }

    pub fn patch_i32_le(&mut self, offset: usize, value: i32) {
        let bytes = value.to_le_bytes();
        self.code[offset..offset + 4].copy_from_slice(&bytes);
    }

    pub fn push_rbp(&mut self) {
        self.push_u8(0x55);
    }

    pub fn pop_rbp(&mut self) {
        self.push_u8(0x5D);
    }

    pub fn mov_rbp_rsp(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]);
    }

    pub fn mov_rsp_rbp(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xEC]);
    }

    pub fn sub_rsp_imm32(&mut self, imm: u32) {
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC]);
        self.push_u32_le(imm);
    }

    pub fn add_rsp_imm32(&mut self, imm: u32) {
        self.code.extend_from_slice(&[0x48, 0x81, 0xC4]);
        self.push_u32_le(imm);
    }

    pub fn ret(&mut self) {
        self.push_u8(0xC3);
    }

    pub fn call_rel32(&mut self, rel: i32) {
        self.push_u8(0xE8);
        self.push_i32_le(rel);
    }

    pub fn call_rel32_placeholder(&mut self) -> usize {
        self.push_u8(0xE8);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    pub fn syscall(&mut self) {
        self.code.extend_from_slice(&[0x0F, 0x05]);
    }

    pub fn xor_eax_eax(&mut self) {
        self.code.extend_from_slice(&[0x31, 0xC0]);
    }

    pub fn mov_rax_imm64(&mut self, imm: u64) {
        self.code.extend_from_slice(&[0x48, 0xB8]);
        self.push_u64_le(imm);
    }

    pub fn mov_rdi_imm64(&mut self, imm: u64) {
        self.code.extend_from_slice(&[0x48, 0xBF]);
        self.push_u64_le(imm);
    }

    pub fn mov_rsi_imm64(&mut self, imm: u64) {
        self.code.extend_from_slice(&[0x48, 0xBE]);
        self.push_u64_le(imm);
    }

    pub fn mov_rdx_imm64(&mut self, imm: u64) {
        self.code.extend_from_slice(&[0x48, 0xBA]);
        self.push_u64_le(imm);
    }

    pub fn mov_rdi_imm32(&mut self, imm: u32) {
        self.code.extend_from_slice(&[0x48, 0xC7, 0xC7]);
        self.push_u32_le(imm);
    }

    pub fn mov_rax_imm32(&mut self, imm: u32) {
        self.code.extend_from_slice(&[0x48, 0xC7, 0xC0]);
        self.push_u32_le(imm);
    }

    pub fn mov_rdx_imm32(&mut self, imm: u32) {
        self.code.extend_from_slice(&[0x48, 0xC7, 0xC2]);
        self.push_u32_le(imm);
    }

    pub fn mov_rsi_rax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xC6]);
    }

    pub fn mov_rdi_rax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xC7]);
    }

    pub fn mov_rax_rdi(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xF8]);
    }

    pub fn mov_rcx_imm64(&mut self, imm: u64) {
        self.code.extend_from_slice(&[0x48, 0xB9]);
        self.push_u64_le(imm);
    }

    pub fn mov_r8_imm64(&mut self, imm: u64) {
        self.code.extend_from_slice(&[0x49, 0xB8]);
        self.push_u64_le(imm);
    }

    pub fn lea_rdi_rip_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0x48, 0x8D, 0x3D]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    pub fn lea_rsi_rip_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0x48, 0x8D, 0x35]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    pub fn lea_rax_rip_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0x48, 0x8D, 0x05]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // movsd xmm0, [rbp - offset]
    pub fn movsd_xmm0_rbp_disp32(&mut self, disp: i32) {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x85]);
        self.push_i32_le(disp);
    }

    // movsd [rbp - offset], xmm0
    pub fn movsd_rbp_disp32_xmm0(&mut self, disp: i32) {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x85]);
        self.push_i32_le(disp);
    }

    // movsd xmm1, [rbp - offset]
    pub fn movsd_xmm1_rbp_disp32(&mut self, disp: i32) {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x8D]);
        self.push_i32_le(disp);
    }

    // movsd xmm0, [rip + disp32]
    pub fn movsd_xmm0_rip_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x05]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // addsd xmm0, xmm1
    pub fn addsd_xmm0_xmm1(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xC1]);
    }

    // subsd xmm0, xmm1
    pub fn subsd_xmm0_xmm1(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x5C, 0xC1]);
    }

    // mulsd xmm0, xmm1
    pub fn mulsd_xmm0_xmm1(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x59, 0xC1]);
    }

    // divsd xmm0, xmm1
    pub fn divsd_xmm0_xmm1(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x5E, 0xC1]);
    }

    // xorpd xmm1, xmm1
    pub fn xorpd_xmm1_xmm1(&mut self) {
        self.code.extend_from_slice(&[0x66, 0x0F, 0x57, 0xC9]);
    }

    // xorpd xmm0, [rip+rel32] (for negation using sign bit mask)
    pub fn xorpd_xmm0_rip_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0x66, 0x0F, 0x57, 0x05]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // cvttsd2si rax, xmm0
    pub fn cvttsd2si_rax_xmm0(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]);
    }

    // cvtsi2sd xmm0, rax
    pub fn cvtsi2sd_xmm0_rax(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xC0]);
    }

    // cvtsi2sd xmm1, rax
    pub fn cvtsi2sd_xmm1_rax(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xC8]);
    }

    // ucomisd xmm0, xmm1
    pub fn ucomisd_xmm0_xmm1(&mut self) {
        self.code.extend_from_slice(&[0x66, 0x0F, 0x2E, 0xC1]);
    }

    // movapd xmm1, xmm0
    pub fn movapd_xmm1_xmm0(&mut self) {
        self.code.extend_from_slice(&[0x66, 0x0F, 0x28, 0xC8]);
    }

    // movapd xmm0, xmm1
    pub fn movapd_xmm0_xmm1(&mut self) {
        self.code.extend_from_slice(&[0x66, 0x0F, 0x28, 0xC1]);
    }

    // mov [rdi], al
    pub fn mov_rdi_deref_al(&mut self) {
        self.code.extend_from_slice(&[0x88, 0x07]);
    }

    // mov al, [rdi]
    pub fn mov_al_rdi_deref(&mut self) {
        self.code.extend_from_slice(&[0x8A, 0x07]);
    }

    // mov [rdi + offset], al  (with disp8)
    pub fn mov_rdi_disp8_al(&mut self, disp: i8) {
        self.code.extend_from_slice(&[0x88, 0x47, disp as u8]);
    }

    // inc rdi
    pub fn inc_rdi(&mut self) {
        self.code.extend_from_slice(&[0x48, 0xFF, 0xC7]);
    }

    // dec rcx
    pub fn dec_rcx(&mut self) {
        self.code.extend_from_slice(&[0x48, 0xFF, 0xC9]);
    }

    // test rax, rax
    pub fn test_rax_rax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x85, 0xC0]);
    }

    // test rcx, rcx
    pub fn test_rcx_rcx(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x85, 0xC9]);
    }

    // cmp rax, imm8
    pub fn cmp_rax_imm8(&mut self, imm: u8) {
        self.code.extend_from_slice(&[0x48, 0x83, 0xF8, imm]);
    }

    // cmp al, imm8
    pub fn cmp_al_imm8(&mut self, imm: u8) {
        self.code.extend_from_slice(&[0x3C, imm]);
    }

    // cmp rcx, imm8
    pub fn cmp_rcx_imm8(&mut self, imm: u8) {
        self.code.extend_from_slice(&[0x48, 0x83, 0xF9, imm]);
    }

    // add rdi, imm8
    pub fn add_rdi_imm8(&mut self, imm: u8) {
        self.code.extend_from_slice(&[0x48, 0x83, 0xC7, imm]);
    }

    // add rax, imm8
    pub fn add_rax_imm8(&mut self, imm: u8) {
        self.code.extend_from_slice(&[0x48, 0x83, 0xC0, imm]);
    }

    // sub rax, imm8
    pub fn sub_rax_imm8(&mut self, imm: u8) {
        self.code.extend_from_slice(&[0x48, 0x83, 0xE8, imm]);
    }

    // add al, imm8
    pub fn add_al_imm8(&mut self, imm: u8) {
        self.code.extend_from_slice(&[0x04, imm]);
    }

    // xor rdx, rdx
    pub fn xor_rdx_rdx(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x31, 0xD2]);
    }

    // div r8 (unsigned: rdx:rax / r8)
    pub fn div_r8(&mut self) {
        self.code.extend_from_slice(&[0x49, 0xF7, 0xF0]);
    }

    // push rax
    pub fn push_rax(&mut self) {
        self.push_u8(0x50);
    }

    // push rdx
    pub fn push_rdx(&mut self) {
        self.push_u8(0x52);
    }

    // push rdi
    pub fn push_rdi(&mut self) {
        self.push_u8(0x57);
    }

    // push rsi
    pub fn push_rsi(&mut self) {
        self.push_u8(0x56);
    }

    // push rcx
    pub fn push_rcx(&mut self) {
        self.push_u8(0x51);
    }

    // pop rax
    pub fn pop_rax(&mut self) {
        self.push_u8(0x58);
    }

    // pop rdx
    pub fn pop_rdx(&mut self) {
        self.push_u8(0x5A);
    }

    // pop rdi
    pub fn pop_rdi(&mut self) {
        self.push_u8(0x5F);
    }

    // pop rsi
    pub fn pop_rsi(&mut self) {
        self.push_u8(0x5E);
    }

    // pop rcx
    pub fn pop_rcx(&mut self) {
        self.push_u8(0x59);
    }

    // jne rel8
    pub fn jne_rel8(&mut self, rel: i8) {
        self.code.extend_from_slice(&[0x75, rel as u8]);
    }

    // je rel8
    pub fn je_rel8(&mut self, rel: i8) {
        self.code.extend_from_slice(&[0x74, rel as u8]);
    }

    // jmp rel8
    pub fn jmp_rel8(&mut self, rel: i8) {
        self.code.extend_from_slice(&[0xEB, rel as u8]);
    }

    // jae rel8 (unsigned >=)
    pub fn jae_rel8(&mut self, rel: i8) {
        self.code.extend_from_slice(&[0x73, rel as u8]);
    }

    // jb rel8 (unsigned <)
    pub fn jb_rel8(&mut self, rel: i8) {
        self.code.extend_from_slice(&[0x72, rel as u8]);
    }

    // jp rel8 (parity - for NaN checks)
    pub fn jp_rel8(&mut self, rel: i8) {
        self.code.extend_from_slice(&[0x7A, rel as u8]);
    }

    // jne rel32
    pub fn jne_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0x0F, 0x85]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // je rel32
    pub fn je_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0x0F, 0x84]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // jmp rel32
    pub fn jmp_rel32(&mut self) -> usize {
        self.push_u8(0xE9);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // jae rel32
    pub fn jae_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0x0F, 0x83]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // jp rel32
    pub fn jp_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0x0F, 0x8A]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    pub fn patch_rel32(&mut self, fixup_offset: usize) {
        let target = self.pos() as i32;
        let rel = target - (fixup_offset as i32 + 4);
        self.patch_i32_le(fixup_offset, rel);
    }

    // neg rax
    pub fn neg_rax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0xF7, 0xD8]);
    }

    // mov rax, [rip + disp32]
    pub fn mov_rax_rip_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0x48, 0x8B, 0x05]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // mov [rip + disp32], rax
    pub fn mov_rip_rel32_rax(&mut self) -> usize {
        self.code.extend_from_slice(&[0x48, 0x89, 0x05]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // add rax, rsi
    pub fn add_rax_rsi(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x01, 0xF0]);
    }

    // mov rsi, [rip + disp32]
    pub fn mov_rsi_rip_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0x48, 0x8B, 0x35]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // mov rdx, rax
    pub fn mov_rdx_rax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xC2]);
    }

    // mov rcx, rax
    pub fn mov_rcx_rax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xC1]);
    }

    // mov rax, rcx
    pub fn mov_rax_rcx(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xC8]);
    }

    // sub rdx, rax
    pub fn sub_rdx_rax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x29, 0xC2]);
    }

    // cmp rdx, rsi
    pub fn cmp_rdx_rsi(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x39, 0xF2]);
    }

    // rep movsb (rcx bytes from [rsi] to [rdi])
    pub fn rep_movsb(&mut self) {
        self.code.extend_from_slice(&[0xF3, 0xA4]);
    }

    // mov [rbp + disp32], rax
    pub fn mov_rbp_disp32_rax(&mut self, disp: i32) {
        self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
        self.push_i32_le(disp);
    }

    // mov rax, [rbp + disp32]
    pub fn mov_rax_rbp_disp32(&mut self, disp: i32) {
        self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
        self.push_i32_le(disp);
    }

    // mov [rbp + disp32], rsi
    pub fn mov_rbp_disp32_rsi(&mut self, disp: i32) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xB5]);
        self.push_i32_le(disp);
    }

    // mov rsi, [rbp + disp32]
    pub fn mov_rsi_rbp_disp32(&mut self, disp: i32) {
        self.code.extend_from_slice(&[0x48, 0x8B, 0xB5]);
        self.push_i32_le(disp);
    }

    // mov rdi, [rbp + disp32]
    pub fn mov_rdi_rbp_disp32(&mut self, disp: i32) {
        self.code.extend_from_slice(&[0x48, 0x8B, 0xBD]);
        self.push_i32_le(disp);
    }

    // mov rdx, [rbp + disp32]
    pub fn mov_rdx_rbp_disp32(&mut self, disp: i32) {
        self.code.extend_from_slice(&[0x48, 0x8B, 0x95]);
        self.push_i32_le(disp);
    }

    // add rdi, rax
    pub fn add_rdi_rax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x01, 0xC7]);
    }

    // mov rcx, rsi
    pub fn mov_rcx_rsi(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xF1]);
    }

    // mov rcx, rdx
    pub fn mov_rcx_rdx(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xD1]);
    }

    // movsd xmm0, xmm1
    pub fn movsd_xmm0_xmm1(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0xC1]);
    }

    // subsd xmm0, xmm0
    pub fn subsd_xmm0_xmm0(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x5C, 0xC0]);
    }

    // mulsd xmm0, [rip + disp32]
    pub fn mulsd_xmm0_rip_rel32(&mut self) -> usize {
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x59, 0x05]);
        let offset = self.pos();
        self.push_i32_le(0);
        offset
    }

    // nop
    pub fn nop(&mut self) {
        self.push_u8(0x90);
    }
}
