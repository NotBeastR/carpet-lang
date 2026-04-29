use carpet_codegen::output::{CodegenOutput, RelocKind};

const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u8 = 1;
const ELFOSABI_NONE: u8 = 0;
const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 62;
const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;
const EHDR_SIZE: usize = 64;
const PHDR_SIZE: usize = 56;

const BASE_ADDR: u64 = 0x400000;
const PAGE_SIZE: u64 = 0x1000;

pub fn link_elf(output: &CodegenOutput) -> Vec<u8> {
    let num_phdrs = 3u16; // code+rodata, bss, (could add more later)
    let headers_end = EHDR_SIZE + PHDR_SIZE * num_phdrs as usize;

    // Text section starts right after headers (no page-alignment gap)
    let text_offset = headers_end;
    let text_size = output.text.len();

    // Rodata follows text, 16-byte aligned
    let rodata_offset = align_up(text_offset + text_size, 16);
    let rodata_size = output.rodata.len();

    // Total file size for the first segment
    let seg1_filesz = rodata_offset + rodata_size;
    let seg1_memsz = seg1_filesz;

    // BSS goes into a separate segment at a new page-aligned virtual address
    let seg1_vaddr = BASE_ADDR;
    let bss_vaddr = align_up_u64(BASE_ADDR + seg1_filesz as u64, PAGE_SIZE);
    let bss_size = output.bss_size;

    // Entry point
    let text_vaddr = BASE_ADDR + text_offset as u64;
    let rodata_vaddr = BASE_ADDR + rodata_offset as u64;
    let entry_vaddr = text_vaddr + output.entry_offset;

    let mut binary = Vec::new();

    // ELF header
    write_elf_header(&mut binary, entry_vaddr, num_phdrs);

    // PHDR 1: Code + Rodata (R|X) - starts at file offset 0
    write_phdr(
        &mut binary,
        PT_LOAD,
        PF_R | PF_X,
        0,                  // p_offset
        seg1_vaddr,         // p_vaddr
        seg1_filesz as u64, // p_filesz
        seg1_memsz as u64,  // p_memsz
        PAGE_SIZE,          // p_align
    );

    // PHDR 2: BSS (R|W)
    write_phdr(
        &mut binary,
        PT_LOAD,
        PF_R | PF_W,
        0,         // p_offset (no file backing)
        bss_vaddr, // p_vaddr
        0,         // p_filesz
        bss_size,  // p_memsz
        PAGE_SIZE, // p_align
    );

    // PHDR 3: Writable data (placeholder, zero-size, for future use)
    write_phdr(
        &mut binary,
        PT_LOAD,
        PF_R | PF_W,
        0,
        bss_vaddr + align_up_u64(bss_size, PAGE_SIZE),
        0,
        0,
        PAGE_SIZE,
    );

    // Write text section
    while binary.len() < text_offset {
        binary.push(0);
    }

    let mut text = output.text.clone();
    apply_relocations(
        &mut text,
        &output.relocations,
        text_vaddr,
        rodata_vaddr,
        bss_vaddr,
        output,
    );
    binary.extend_from_slice(&text);

    // Pad to rodata offset
    while binary.len() < rodata_offset {
        binary.push(0);
    }

    // Write rodata
    binary.extend_from_slice(&output.rodata);

    binary
}

fn apply_relocations(
    text: &mut [u8],
    relocations: &[carpet_codegen::output::Relocation],
    text_vaddr: u64,
    rodata_vaddr: u64,
    bss_vaddr: u64,
    output: &CodegenOutput,
) {
    let buffer_vaddr = bss_vaddr;
    let buffer_pos_vaddr = bss_vaddr + 131072;

    for reloc in relocations {
        let instr_end = reloc.offset + 4;
        let rip_value = text_vaddr + instr_end as u64;

        let target_addr = match reloc.kind {
            RelocKind::BssBufferAddr => buffer_vaddr,
            RelocKind::BssBufferPosAddr => buffer_pos_vaddr,
            RelocKind::RodataOffset(off) => rodata_vaddr + off as u64,
            RelocKind::SignMaskAddr => rodata_vaddr + output.sign_mask_offset() as u64,
            RelocKind::TenConstAddr => rodata_vaddr + output.ten_const_offset() as u64,
        };

        let rel = (target_addr as i64 - rip_value as i64) as i32;
        text[reloc.offset..reloc.offset + 4].copy_from_slice(&rel.to_le_bytes());
    }
}

fn write_elf_header(buf: &mut Vec<u8>, entry: u64, phnum: u16) {
    buf.extend_from_slice(&ELF_MAGIC);
    buf.push(ELFCLASS64);
    buf.push(ELFDATA2LSB);
    buf.push(EV_CURRENT);
    buf.push(ELFOSABI_NONE);
    buf.extend_from_slice(&[0u8; 8]);
    buf.extend_from_slice(&ET_EXEC.to_le_bytes());
    buf.extend_from_slice(&EM_X86_64.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes()); // e_version
    buf.extend_from_slice(&entry.to_le_bytes());
    buf.extend_from_slice(&(EHDR_SIZE as u64).to_le_bytes()); // e_phoff
    buf.extend_from_slice(&0u64.to_le_bytes()); // e_shoff
    buf.extend_from_slice(&0u32.to_le_bytes()); // e_flags
    buf.extend_from_slice(&(EHDR_SIZE as u16).to_le_bytes());
    buf.extend_from_slice(&(PHDR_SIZE as u16).to_le_bytes());
    buf.extend_from_slice(&phnum.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // e_shentsize
    buf.extend_from_slice(&0u16.to_le_bytes()); // e_shnum
    buf.extend_from_slice(&0u16.to_le_bytes()); // e_shstrndx
}

#[allow(clippy::too_many_arguments)]
fn write_phdr(
    buf: &mut Vec<u8>,
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
) {
    buf.extend_from_slice(&p_type.to_le_bytes());
    buf.extend_from_slice(&p_flags.to_le_bytes());
    buf.extend_from_slice(&p_offset.to_le_bytes());
    buf.extend_from_slice(&p_vaddr.to_le_bytes());
    buf.extend_from_slice(&p_vaddr.to_le_bytes()); // p_paddr = p_vaddr
    buf.extend_from_slice(&p_filesz.to_le_bytes());
    buf.extend_from_slice(&p_memsz.to_le_bytes());
    buf.extend_from_slice(&p_align.to_le_bytes());
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

fn align_up_u64(value: u64, align: u64) -> u64 {
    (value + align - 1) & !(align - 1)
}
