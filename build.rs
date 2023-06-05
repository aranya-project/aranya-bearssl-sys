#![allow(unstable_name_collisions)]

use {
    bindgen::{EnumVariation, Formatter, MacroTypeVariation},
    std::{
        env, error, fmt,
        path::{Path, PathBuf},
        process::{Command, ExitStatus},
        result, str,
    },
};

type Result<T> = result::Result<T, Box<dyn error::Error>>;

#[derive(Debug)]
struct ExitStatusError(ExitStatus);

impl fmt::Display for ExitStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "process exited unsuccessfully: {}", self.0)
    }
}

impl error::Error for ExitStatusError {}

trait StatusResult {
    fn exit_ok(&self) -> result::Result<(), ExitStatusError>;
}

impl StatusResult for ExitStatus {
    fn exit_ok(&self) -> result::Result<(), ExitStatusError> {
        if self.success() {
            Ok(())
        } else {
            Err(ExitStatusError(*self))
        }
    }
}

/// The env var that has the directory we search for precompiled
/// BearSSL files.
const BEARSSL_PRECOMPILED_PATH_VAR: &str = "BEARSSL_PRECOMPILED_PATH";
/// The env var that has the directory we search for BearSSL
/// source files.
const BEARSSL_SOURCE_PATH_VAR: &str = "BEARSSL_SOURCE_PATH";
/// The env var that has the directory we search for BearSSL
/// header files.
const BEARSSL_INCLUDE_PATH_VAR: &str = "BEARSSL_INCLUDE_PATH";
/// The env var that has the git hash we checkout if neither
/// BEARSSL_PRECOMPILED_PATH nor BEARSSL_SOURCE_PATH are provided.
const BEARSSL_GIT_HASH_VAR: &str = "BEARSSL_GIT_HASH";
/// The git hash we checkout if BEARSSL_GIT_HASH is unset.
///
/// This is master as of 2023/06/05.
const BEARSSL_GIT_HASH: &str = "79c060eea3eea1257797f15ea1608a9a9923aa6f";
/// The directory the baked-in BearSSL sources are cloned into.
const BEARSSL_DEPS_PATH: &str = "deps/bearssl";

enum Sources {
    Precompiled(PathBuf),
    Raw(PathBuf),
}

fn find_bearssl_sources() -> Result<Sources> {
    println!("cargo:rerun-if-env-changed={BEARSSL_PRECOMPILED_PATH_VAR}");
    if let Ok(dir) = env::var(BEARSSL_PRECOMPILED_PATH_VAR) {
        let path = Path::new(&dir);
        if path.exists() {
            return Ok(Sources::Precompiled(path.to_owned()));
        }
    }

    println!("cargo:rerun-if-env-changed={BEARSSL_SOURCE_PATH_VAR}");
    if let Ok(dir) = env::var(BEARSSL_SOURCE_PATH_VAR) {
        let path = Path::new(&dir);
        if path.exists() {
            return Ok(Sources::Raw(path.to_owned()));
        }
    }

    println!("cargo:rerun-if-env-changed={BEARSSL_GIT_HASH}");
    let path = Path::new(BEARSSL_DEPS_PATH);
    if !path.join("Makefile").exists() {
        println!("cargo:warning=fetching BearSSL");
        Command::new("git")
            .arg("clone")
            .arg("https://www.bearssl.org/git/BearSSL")
            .arg(BEARSSL_DEPS_PATH)
            .status()?
            .exit_ok()?;
        let hash = env::var(BEARSSL_GIT_HASH_VAR).unwrap_or(BEARSSL_GIT_HASH.to_owned());
        Command::new("git")
            .arg("checkout")
            .arg(hash)
            .current_dir(BEARSSL_DEPS_PATH)
            .status()?
            .exit_ok()?;
    }
    Ok(Sources::Raw(path.to_owned()))
}

fn into_string<P>(path: P) -> String
where
    P: AsRef<Path>,
{
    path.as_ref().to_str().unwrap().to_owned()
}

fn main() -> Result<()> {
    let (src_dir, build_dir) = match find_bearssl_sources()? {
        Sources::Precompiled(dir) => (dir.clone(), dir),
        Sources::Raw(dir) => {
            println!("cargo:warning=compiling BearSSL at {:?}", dir);

            Command::new("make")
                .current_dir(BEARSSL_DEPS_PATH)
                .status()?
                .exit_ok()?;

            (dir.clone(), dir.join("build"))
        }
    };

    let lib_dir = build_dir.clone();
    println!(
        "cargo:rustc-link-search=native={}",
        lib_dir.as_path().to_str().unwrap()
    );

    println!("cargo:rustc-link-lib=static=bearssl");

    if env::var("CARGO_CFG_TARGET_OS")? == "macos" {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-undefined,dynamic_lookup");
    }

    println!("cargo:rerun-if-env-changed={BEARSSL_INCLUDE_PATH_VAR}");
    let include_path = env::var(BEARSSL_INCLUDE_PATH_VAR)
        .map_or_else(|_| src_dir.join("inc"), |v| Path::new(&v).to_owned());

    let mut builder = bindgen::Builder::default()
        .array_pointers_in_arguments(true)
        .clang_args(&["-I", into_string(include_path.clone()).as_str()])
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

    let headers = [
        "bearssl.h",
        "bearssl_aead.h",
        "bearssl_block.h",
        "bearssl_ec.h",
        "bearssl_hash.h",
        "bearssl_hmac.h",
        "bearssl_kdf.h",
        "bearssl_pem.h",
        "bearssl_prf.h",
        "bearssl_rand.h",
        "bearssl_rsa.h",
        "bearssl_ssl.h",
        "bearssl_x509.h",
    ];
    for header in &headers {
        builder = builder.header(include_path.join(header).to_str().unwrap());
    }

    let bindings = builder.generate().expect("unable to generate bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR")?);
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("unable to write bindings");
    Ok(())
}
