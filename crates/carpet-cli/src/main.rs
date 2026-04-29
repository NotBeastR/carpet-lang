use std::env;
use std::fs;
use std::process;

use carpet::error::CarpetError;
use carpet::lexer::Lexer;
use carpet::parser::Parser;
use carpet_codegen::emit::Emitter;
use carpet_codegen::target::Target;
use carpet_ir::lower::Lowerer;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Flying Carpet v0.0.1 — The Carpet Language Engine");
        eprintln!("Usage: carpet <source.cpt> [--target <linux|macos|windows>] [-o <output>]");
        process::exit(1);
    }

    let mut source_file = None;
    let mut output_file = None;
    let mut target = Target::from_host();
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --target requires an argument (linux, macos, windows)");
                    process::exit(1);
                }
                target = match args[i].as_str() {
                    "linux" => Target::LinuxX86_64,
                    "macos" => Target::MacOSX86_64,
                    "windows" => Target::WindowsX86_64,
                    other => {
                        eprintln!("error: unknown target '{other}'. Use linux, macos, or windows");
                        process::exit(1);
                    }
                };
            }
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: -o requires an output file path");
                    process::exit(1);
                }
                output_file = Some(args[i].clone());
            }
            _ => {
                if source_file.is_some() {
                    eprintln!("error: unexpected argument '{}'", args[i]);
                    process::exit(1);
                }
                source_file = Some(args[i].clone());
            }
        }
        i += 1;
    }

    let source_path = match source_file {
        Some(p) => p,
        None => {
            eprintln!("error: no source file provided");
            process::exit(1);
        }
    };

    let out_path = output_file.unwrap_or_else(|| {
        let stem = source_path.strip_suffix(".cpt").unwrap_or(&source_path);
        match target {
            Target::WindowsX86_64 => format!("{stem}.exe"),
            _ => stem.to_string(),
        }
    });

    let source = match fs::read_to_string(&source_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read '{}': {}", source_path, e);
            process::exit(1);
        }
    };

    if let Err(e) = compile(&source, &source_path, &out_path, target) {
        eprintln!("{}", e.format_with_source(&source, &source_path));
        process::exit(1);
    }
}

fn compile(source: &str, _filename: &str, output: &str, target: Target) -> Result<(), CarpetError> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;

    let mut parser = Parser::new(tokens);
    let program = parser.parse()?;

    let lowerer = Lowerer::new();
    let module = lowerer.lower(&program)?;

    let emitter = Emitter::new(target);
    let codegen_output = emitter.emit(&module);

    let binary = mirage::link(&codegen_output);

    fs::write(output, &binary).map_err(|e| {
        CarpetError::new(
            carpet::error::ErrorKind::UnexpectedCharacter,
            format!("could not write output file '{}': {}", output, e),
            carpet::span::Span::new(0, 0, 1, 1),
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(output)
            .map_err(|e| {
                CarpetError::new(
                    carpet::error::ErrorKind::UnexpectedCharacter,
                    format!("could not read permissions: {}", e),
                    carpet::span::Span::new(0, 0, 1, 1),
                )
            })?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(output, perms).map_err(|e| {
            CarpetError::new(
                carpet::error::ErrorKind::UnexpectedCharacter,
                format!("could not set permissions: {}", e),
                carpet::span::Span::new(0, 0, 1, 1),
            )
        })?;
    }

    Ok(())
}
