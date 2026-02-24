#![allow(unstable_name_collisions)]

use std::{
    env,
    path::{Path, PathBuf},
};

use bindgen::{EnumVariation, Formatter, MacroTypeVariation};

type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

fn find(root: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    glob::glob(root.join(pattern).to_str().unwrap())?
        .collect::<Result<_, _>>()
        .map_err(Into::into)
}

fn main() -> Result<()> {
    let src_dir = Path::new("vendor");

    println!("cargo:warning=compiling BearSSL at {:?}", src_dir);

    let mut build = cc::Build::new();
    build
        .include(src_dir.join("inc"))
        .include(src_dir.join("src"))
        .files(find(src_dir, "src/**/*.c")?)
        .opt_level_str("s");

    match env::var("CARGO_CFG_TARGET_OS")?.as_str() {
        "linux" => {
            build
                .flag("-Wstack-protector")
                .flag("-fstack-protector-all");
        }
        "vxworks" => {
            build
                .static_flag(true)
                .flag("-mrtp")
                .flag("-msoft-float")
                .flag("-mstrict-align")
                .flag("-mregnames")
                .flag("-fno-split-stack")
                .flag("-fno-strict-aliasing")
                .flag("-Wstack-protector")
                .flag("-fstack-protector-all")
                .flag("-gdwarf-2")
                .define("_C99", None)
                .define("_HAS_C9X", None);
        }
        _ => {}
    }

    build.compile("bearssl");

    let include_path = src_dir.join("inc");

    let mut builder = bindgen::Builder::default()
        .header(include_path.join("bearssl.h").to_str().unwrap())
        .allowlist_function("br_.*")
        .allowlist_type("br_.*")
        .allowlist_var("(br|BR)_.*")
        .array_pointers_in_arguments(true)
        .clang_args(&["-I", include_path.to_str().unwrap()])
        .ctypes_prefix("::core::ffi")
        .default_enum_style(EnumVariation::NewType {
            is_bitfield: false,
            is_global: false,
        })
        .default_macro_constant_type(MacroTypeVariation::Signed)
        .derive_copy(true)
        .derive_debug(true)
        .derive_default(true)
        .derive_eq(true)
        .enable_function_attribute_detection()
        .fit_macro_constants(false)
        .formatter(Formatter::Rustfmt)
        .generate_comments(true)
        .layout_tests(true)
        .merge_extern_blocks(true)
        .prepend_enum_name(true)
        .size_t_is_usize(true)
        .time_phases(true)
        .use_core();

    let target = env::var("TARGET")?;
    match target.as_ref() {
        // bindgen produces alignment tests that cause undefined behavior [1]
        // when applied to explicitly unaligned types like OSUnalignedU64.
        //
        // There is no way to disable these tests for only some types
        // and it's not nice to suppress warnings for the entire crate,
        // so let's disable all alignment tests and hope for the best.
        //
        // [1]: https://github.com/rust-lang/rust-bindgen/issues/1651
        "aarch64-apple-ios" | "aarch64-apple-ios-sim" => {
            builder = builder.layout_tests(false);
        }
        _ => {}
    }

    let bindings = builder.generate().expect("unable to generate bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR")?);
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("unable to write bindings");
    Ok(())
}
