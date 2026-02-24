#![allow(unstable_name_collisions)]

use std::{
    env,
    path::{Path, PathBuf},
};

use bindgen::{EnumVariation, MacroTypeVariation};

fn find(root: &Path, pattern: &str) -> impl Iterator<Item: AsRef<Path>> {
    glob::glob(root.join(pattern).to_str().unwrap())
        .expect("failed to glob")
        .map(|r| r.expect("bad glob entry"))
}

fn build_c_library(src_dir: &Path) {
    let mut build = cc::Build::new();
    build
        .include(src_dir.join("inc"))
        .include(src_dir.join("src"))
        .files(find(src_dir, "src/**/*.c"))
        .opt_level_str("s");

    match env::var("CARGO_CFG_TARGET_OS")
        .expect("invalid target OS")
        .as_str()
    {
        "linux" => {
            build
                .flag("-Wstack-protector")
                .flag("-fstack-protector-all");
        }
        "vxworks" => {
            build
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
}

fn build_rust_interface(src_dir: &Path) {
    let include_path = src_dir.join("inc");

    let builder = bindgen::Builder::default()
        .header(include_path.join("bearssl.h").to_str().unwrap())
        .allowlist_function("br_.*")
        .allowlist_type("br_.*")
        .allowlist_var("(br|BR)_.*")
        .array_pointers_in_arguments(true)
        .clang_args(&["-I", include_path.to_str().unwrap()])
        .default_enum_style(EnumVariation::NewType {
            is_bitfield: false,
            is_global: false,
        })
        .default_macro_constant_type(MacroTypeVariation::Signed)
        .derive_default(true)
        .derive_eq(true)
        .enable_function_attribute_detection()
        .merge_extern_blocks(true)
        .time_phases(true)
        .use_core();

    let bindings = builder.generate().expect("unable to generate bindings");

    let out_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("unable to write bindings");
}

fn main() {
    let src = Path::new("vendor");
    std::thread::scope(|s| {
        s.spawn(|| build_c_library(src));
        s.spawn(|| build_rust_interface(src));
    });
}
