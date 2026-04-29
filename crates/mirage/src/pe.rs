use carpet_codegen::output::{CodegenOutput, RelocKind};

const DOS_STUB_SIZE: u64 = 64;
const PE_SIGNATURE: u32 = 0x00004550;
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const IMAGE_FILE_EXECUTABLE_IMAGE: u16 = 0x0002;
const IMAGE_FILE_LARGE_ADDRESS_AWARE: u16 = 0x0020;
const PE_OPT_MAGIC_64: u16 = 0x020B;
const SECTION_HEADER_SIZE: u64 = 40;
const IMAGE_BASE: u64 = 0x140000000;
const SECTION_ALIGNMENT: u32 = 0x1000;
const FILE_ALIGNMENT: u32 = 0x200;
const IMAGE_SUBSYSTEM_CONSOLE: u16 = 3;

const IMAGE_SCN_MEM_EXECUTE: u32 = 0x20000000;
const IMAGE_SCN_MEM_READ: u32 = 0x40000000;
const IMAGE_SCN_MEM_WRITE: u32 = 0x80000000;
const IMAGE_SCN_CNT_CODE: u32 = 0x00000020;
const IMAGE_SCN_CNT_INITIALIZED_DATA: u32 = 0x00000040;
const IMAGE_SCN_CNT_UNINITIALIZED_DATA: u32 = 0x00000080;

pub fn link_pe(output: &CodegenOutput) -> Vec<u8> {
    let num_sections = 3u16; // .text, .rdata, .bss
    let num_data_dirs = 16u32;

    let coff_header_size: u64 = 20;
    let opt_header_size: u64 = 112 + num_data_dirs as u64 * 8; // PE32+ optional header

    let headers_raw_size = DOS_STUB_SIZE
        + 4 // PE signature
        + coff_header_size
        + opt_header_size
        + num_sections as u64 * SECTION_HEADER_SIZE;
    let headers_size = align_up_u64(headers_raw_size, FILE_ALIGNMENT as u64);

    let text_rva = SECTION_ALIGNMENT as u64;
    let text_raw_size = align_up_u64(output.text.len() as u64, FILE_ALIGNMENT as u64);
    let text_file_offset = headers_size;

    let rdata_rva = align_up_u64(
        text_rva + output.text.len() as u64,
        SECTION_ALIGNMENT as u64,
    );
    let rdata_raw_size = align_up_u64(output.rodata.len() as u64, FILE_ALIGNMENT as u64);
    let rdata_file_offset = text_file_offset + text_raw_size;

    let bss_rva = align_up_u64(
        rdata_rva + output.rodata.len() as u64,
        SECTION_ALIGNMENT as u64,
    );
    let bss_virtual_size = output.bss_size;

    let entry_rva = text_rva + output.entry_offset;
    let image_size = align_up_u64(bss_rva + bss_virtual_size, SECTION_ALIGNMENT as u64);

    let mut binary = Vec::new();

    // DOS stub (minimal)
    // MZ header
    binary.extend_from_slice(&[0x4D, 0x5A]); // e_magic
    binary.resize(60, 0); // fill DOS header
    write_u32_at(&mut binary, 60, DOS_STUB_SIZE as u32); // e_lfanew
    binary.resize(DOS_STUB_SIZE as usize, 0);

    // PE signature
    write_u32(&mut binary, PE_SIGNATURE);

    // COFF header
    write_u16(&mut binary, IMAGE_FILE_MACHINE_AMD64);
    write_u16(&mut binary, num_sections);
    write_u32(&mut binary, 0); // timestamp
    write_u32(&mut binary, 0); // symbol table pointer
    write_u32(&mut binary, 0); // number of symbols
    write_u16(&mut binary, opt_header_size as u16);
    write_u16(
        &mut binary,
        IMAGE_FILE_EXECUTABLE_IMAGE | IMAGE_FILE_LARGE_ADDRESS_AWARE,
    );

    // Optional header (PE32+)
    write_u16(&mut binary, PE_OPT_MAGIC_64);
    binary.push(1); // major linker version
    binary.push(0); // minor linker version
    write_u32(&mut binary, output.text.len() as u32); // size of code
    write_u32(&mut binary, output.rodata.len() as u32); // size of initialized data
    write_u32(&mut binary, bss_virtual_size as u32); // size of uninitialized data
    write_u32(&mut binary, entry_rva as u32); // address of entry point
    write_u32(&mut binary, text_rva as u32); // base of code

    // PE32+ specific
    write_u64(&mut binary, IMAGE_BASE); // image base
    write_u32(&mut binary, SECTION_ALIGNMENT);
    write_u32(&mut binary, FILE_ALIGNMENT);
    write_u16(&mut binary, 6); // OS version major
    write_u16(&mut binary, 0); // OS version minor
    write_u16(&mut binary, 0); // image version major
    write_u16(&mut binary, 0); // image version minor
    write_u16(&mut binary, 6); // subsystem version major
    write_u16(&mut binary, 0); // subsystem version minor
    write_u32(&mut binary, 0); // win32 version
    write_u32(&mut binary, image_size as u32); // size of image
    write_u32(&mut binary, headers_size as u32); // size of headers
    write_u32(&mut binary, 0); // checksum
    write_u16(&mut binary, IMAGE_SUBSYSTEM_CONSOLE);
    write_u16(&mut binary, 0); // DLL characteristics
    write_u64(&mut binary, 0x100000); // stack reserve
    write_u64(&mut binary, 0x1000); // stack commit
    write_u64(&mut binary, 0x100000); // heap reserve
    write_u64(&mut binary, 0x1000); // heap commit
    write_u32(&mut binary, 0); // loader flags
    write_u32(&mut binary, num_data_dirs);

    // Data directories (all zero for now)
    for _ in 0..num_data_dirs {
        write_u32(&mut binary, 0); // RVA
        write_u32(&mut binary, 0); // Size
    }

    // Section headers
    // .text
    write_section_header(
        &mut binary,
        b".text\0\0\0",
        output.text.len() as u32,
        text_rva as u32,
        text_raw_size as u32,
        text_file_offset as u32,
        IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ,
    );

    // .rdata
    write_section_header(
        &mut binary,
        b".rdata\0\0",
        output.rodata.len() as u32,
        rdata_rva as u32,
        rdata_raw_size as u32,
        rdata_file_offset as u32,
        IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ,
    );

    // .bss
    write_section_header(
        &mut binary,
        b".bss\0\0\0\0",
        bss_virtual_size as u32,
        bss_rva as u32,
        0, // no raw data
        0,
        IMAGE_SCN_CNT_UNINITIALIZED_DATA | IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE,
    );

    // Pad headers
    while binary.len() < headers_size as usize {
        binary.push(0);
    }

    // Apply relocations and write text
    let mut text = output.text.clone();
    apply_relocations(
        &mut text,
        &output.relocations,
        IMAGE_BASE + text_rva,
        IMAGE_BASE + rdata_rva,
        IMAGE_BASE + bss_rva,
        output,
    );
    binary.extend_from_slice(&text);

    // Pad text section
    while binary.len() < (text_file_offset + text_raw_size) as usize {
        binary.push(0);
    }

    // Write rodata
    binary.extend_from_slice(&output.rodata);

    // Pad rdata section
    while binary.len() < (rdata_file_offset + rdata_raw_size) as usize {
        binary.push(0);
    }

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

fn write_section_header(
    buf: &mut Vec<u8>,
    name: &[u8; 8],
    virtual_size: u32,
    virtual_address: u32,
    raw_data_size: u32,
    raw_data_offset: u32,
    characteristics: u32,
) {
    buf.extend_from_slice(name);
    write_u32(buf, virtual_size);
    write_u32(buf, virtual_address);
    write_u32(buf, raw_data_size);
    write_u32(buf, raw_data_offset);
    write_u32(buf, 0); // relocation offset
    write_u32(buf, 0); // line number offset
    write_u16(buf, 0); // num relocations
    write_u16(buf, 0); // num line numbers
    write_u32(buf, characteristics);
}

fn write_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_u32_at(buf: &mut [u8], offset: usize, v: u32) {
    let bytes = v.to_le_bytes();
    buf[offset..offset + 4].copy_from_slice(&bytes);
}

fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn align_up_u64(value: u64, align: u64) -> u64 {
    (value + align - 1) & !(align - 1)
}
