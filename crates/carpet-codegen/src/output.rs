use crate::target::Target;

#[derive(Debug, Clone, Copy)]
pub struct Relocation {
    pub offset: usize,
    pub kind: RelocKind,
}

#[derive(Debug, Clone, Copy)]
pub enum RelocKind {
    BssBufferAddr,
    BssBufferPosAddr,
    RodataOffset(usize),
    SignMaskAddr,
    TenConstAddr,
}

pub struct CodegenOutput {
    pub text: Vec<u8>,
    pub rodata: Vec<u8>,
    pub bss_size: u64,
    pub entry_offset: u64,
    pub relocations: Vec<Relocation>,
    pub target: Target,
    pub sign_mask_rodata_offset: usize,
    pub ten_const_rodata_offset: usize,
}

impl CodegenOutput {
    pub fn sign_mask_offset(&self) -> usize {
        self.sign_mask_rodata_offset
    }

    pub fn ten_const_offset(&self) -> usize {
        self.ten_const_rodata_offset
    }
}
