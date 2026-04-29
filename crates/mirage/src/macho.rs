use carpet_codegen::output::{CodegenOutput, RelocKind};

const MH_MAGIC_64: u32 = 0xFEEDFACF;
const CPU_TYPE_X86_64: u32 = 0x01000007;
const CPU_SUBTYPE_ALL: u32 = 3;
const MH_EXECUTE: u32 = 2;
const MH_NOUNDEFS: u32 = 1;

const LC_SEGMENT_64: u32 = 0x19;
const LC_UNIXTHREAD: u32 = 0x05;

const VM_PROT_READ: u32 = 1;
const VM_PROT_WRITE: u32 = 2;
const VM_PROT_EXECUTE: u32 = 4;

const PAGE_SIZE: u64 = 0x1000;
const BASE_ADDR: u64 = 0x100000000;

const MACH_HEADER_SIZE: u64 = 32;
const SEGMENT_CMD_SIZE: u64 = 72;
const SECTION_SIZE: u64 = 80;

const X86_THREAD_STATE64: u32 = 4;
const X86_THREAD_STATE64_COUNT: u32 = 42;

pub fn link_macho(output: &CodegenOutput) -> Vec<u8> {
    let ncmds = 4u32; // __PAGEZERO, __TEXT, __DATA, LC_UNIXTHREAD
    let text_sections = 1u32; // __text
    let rodata_sections = 1u32; // __const
    let data_sections = 1u32; // __bss

    let unixthread_size = 16 + 4 + 4 + (X86_THREAD_STATE64_COUNT as u64 * 4);

    let load_cmds_size = SEGMENT_CMD_SIZE
        + (SEGMENT_CMD_SIZE + text_sections as u64 * SECTION_SIZE)
        + (SEGMENT_CMD_SIZE + rodata_sections as u64 * SECTION_SIZE)
        + (SEGMENT_CMD_SIZE + data_sections as u64 * SECTION_SIZE)
        + unixthread_size;
    let _header_total = MACH_HEADER_SIZE + load_cmds_size;

    let text_file_offset = align_up(MACH_HEADER_SIZE + load_cmds_size, PAGE_SIZE);
    let text_vaddr = BASE_ADDR + text_file_offset;
    let text_size = output.text.len() as u64;

    let rodata_file_offset = align_up(text_file_offset + text_size, 0x10);
    let rodata_vaddr = BASE_ADDR + rodata_file_offset;
    let rodata_size = output.rodata.len() as u64;

    let text_seg_size = align_up(
        rodata_file_offset + rodata_size - text_file_offset,
        PAGE_SIZE,
    );

    let bss_vaddr = BASE_ADDR + align_up(text_file_offset + text_seg_size, PAGE_SIZE);
    let bss_size = output.bss_size;
    let data_seg_size = align_up(bss_size, PAGE_SIZE);

    let entry_vaddr = text_vaddr + output.entry_offset;

    let mut binary = Vec::new();

    // Mach-O header
    write_u32(&mut binary, MH_MAGIC_64);
    write_u32(&mut binary, CPU_TYPE_X86_64);
    write_u32(&mut binary, CPU_SUBTYPE_ALL);
    write_u32(&mut binary, MH_EXECUTE);
    write_u32(&mut binary, ncmds);
    write_u32(&mut binary, load_cmds_size as u32);
    write_u32(&mut binary, MH_NOUNDEFS);
    write_u32(&mut binary, 0); // reserved

    // LC_SEGMENT_64: __PAGEZERO
    write_u32(&mut binary, LC_SEGMENT_64);
    write_u32(&mut binary, SEGMENT_CMD_SIZE as u32);
    write_segname(&mut binary, "__PAGEZERO");
    write_u64(&mut binary, 0); // vmaddr
    write_u64(&mut binary, BASE_ADDR); // vmsize
    write_u64(&mut binary, 0); // fileoff
    write_u64(&mut binary, 0); // filesize
    write_u32(&mut binary, 0); // maxprot
    write_u32(&mut binary, 0); // initprot
    write_u32(&mut binary, 0); // nsects
    write_u32(&mut binary, 0); // flags

    // LC_SEGMENT_64: __TEXT (contains .text and .rodata)
    write_u32(&mut binary, LC_SEGMENT_64);
    write_u32(
        &mut binary,
        (SEGMENT_CMD_SIZE + text_sections as u64 * SECTION_SIZE) as u32,
    );
    write_segname(&mut binary, "__TEXT");
    write_u64(&mut binary, text_vaddr); // vmaddr
    write_u64(&mut binary, text_seg_size); // vmsize
    write_u64(&mut binary, text_file_offset); // fileoff
    write_u64(&mut binary, text_seg_size); // filesize
    write_u32(&mut binary, VM_PROT_READ | VM_PROT_EXECUTE); // maxprot
    write_u32(&mut binary, VM_PROT_READ | VM_PROT_EXECUTE); // initprot
    write_u32(&mut binary, text_sections); // nsects
    write_u32(&mut binary, 0); // flags

    // Section: __text
    write_sectname(&mut binary, "__text");
    write_segname(&mut binary, "__TEXT");
    write_u64(&mut binary, text_vaddr); // addr
    write_u64(&mut binary, text_size); // size
    write_u32(&mut binary, text_file_offset as u32); // offset
    write_u32(&mut binary, 0); // align (2^0 = 1)
    write_u32(&mut binary, 0); // reloff
    write_u32(&mut binary, 0); // nreloc
    write_u32(&mut binary, 0x80000400); // S_REGULAR | S_ATTR_PURE_INSTRUCTIONS | S_ATTR_SOME_INSTRUCTIONS
    write_u32(&mut binary, 0); // reserved1
    write_u32(&mut binary, 0); // reserved2
    write_u32(&mut binary, 0); // reserved3

    // LC_SEGMENT_64: __RODATA
    write_u32(&mut binary, LC_SEGMENT_64);
    write_u32(
        &mut binary,
        (SEGMENT_CMD_SIZE + rodata_sections as u64 * SECTION_SIZE) as u32,
    );
    write_segname(&mut binary, "__RODATA");
    write_u64(&mut binary, rodata_vaddr);
    write_u64(&mut binary, align_up(rodata_size, PAGE_SIZE));
    write_u64(&mut binary, rodata_file_offset);
    write_u64(&mut binary, rodata_size);
    write_u32(&mut binary, VM_PROT_READ);
    write_u32(&mut binary, VM_PROT_READ);
    write_u32(&mut binary, rodata_sections);
    write_u32(&mut binary, 0);

    // Section: __const
    write_sectname(&mut binary, "__const");
    write_segname(&mut binary, "__RODATA");
    write_u64(&mut binary, rodata_vaddr);
    write_u64(&mut binary, rodata_size);
    write_u32(&mut binary, rodata_file_offset as u32);
    write_u32(&mut binary, 4); // align 2^4 = 16
    write_u32(&mut binary, 0);
    write_u32(&mut binary, 0);
    write_u32(&mut binary, 0); // S_REGULAR
    write_u32(&mut binary, 0);
    write_u32(&mut binary, 0);
    write_u32(&mut binary, 0);

    // LC_SEGMENT_64: __DATA (BSS)
    write_u32(&mut binary, LC_SEGMENT_64);
    write_u32(
        &mut binary,
        (SEGMENT_CMD_SIZE + data_sections as u64 * SECTION_SIZE) as u32,
    );
    write_segname(&mut binary, "__DATA");
    write_u64(&mut binary, bss_vaddr);
    write_u64(&mut binary, data_seg_size);
    write_u64(&mut binary, 0); // no file data
    write_u64(&mut binary, 0);
    write_u32(&mut binary, VM_PROT_READ | VM_PROT_WRITE);
    write_u32(&mut binary, VM_PROT_READ | VM_PROT_WRITE);
    write_u32(&mut binary, data_sections);
    write_u32(&mut binary, 0);

    // Section: __bss
    write_sectname(&mut binary, "__bss");
    write_segname(&mut binary, "__DATA");
    write_u64(&mut binary, bss_vaddr);
    write_u64(&mut binary, bss_size);
    write_u32(&mut binary, 0); // no file offset
    write_u32(&mut binary, 0);
    write_u32(&mut binary, 0);
    write_u32(&mut binary, 0);
    write_u32(&mut binary, 1); // S_ZEROFILL
    write_u32(&mut binary, 0);
    write_u32(&mut binary, 0);
    write_u32(&mut binary, 0);

    // LC_UNIXTHREAD
    write_u32(&mut binary, LC_UNIXTHREAD);
    write_u32(&mut binary, unixthread_size as u32);
    write_u32(&mut binary, X86_THREAD_STATE64);
    write_u32(&mut binary, X86_THREAD_STATE64_COUNT);

    // x86_64 thread state: 21 registers (rax..gs), each 64-bit
    // rip is register index 16
    for i in 0..21u32 {
        if i == 16 {
            write_u64(&mut binary, entry_vaddr); // rip
        } else {
            write_u64(&mut binary, 0);
        }
    }

    // Pad to text offset
    while binary.len() < text_file_offset as usize {
        binary.push(0);
    }

    // Apply relocations
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

    // Pad to rodata
    while binary.len() < rodata_file_offset as usize {
        binary.push(0);
    }
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

fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_segname(buf: &mut Vec<u8>, name: &str) {
    let mut seg = [0u8; 16];
    let bytes = name.as_bytes();
    let len = bytes.len().min(16);
    seg[..len].copy_from_slice(&bytes[..len]);
    buf.extend_from_slice(&seg);
}

fn write_sectname(buf: &mut Vec<u8>, name: &str) {
    write_segname(buf, name); // same 16-byte format
}

fn align_up(value: u64, align: u64) -> u64 {
    (value + align - 1) & !(align - 1)
}
