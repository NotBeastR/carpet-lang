pub mod elf;
pub mod macho;
pub mod pe;

use carpet_codegen::output::CodegenOutput;
use carpet_codegen::target::Target;

pub fn link(output: &CodegenOutput) -> Vec<u8> {
    match output.target {
        Target::LinuxX86_64 => elf::link_elf(output),
        Target::MacOSX86_64 => macho::link_macho(output),
        Target::WindowsX86_64 => pe::link_pe(output),
    }
}
