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

type Result<T, E = Box<dyn error::Error>> = result::Result<T, E>;

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

    println!("cargo:rerun-if-env-changed={BEARSSL_GIT_HASH_VAR}");
    let path = Path::new(&env::var("OUT_DIR")?).join(BEARSSL_DEPS_PATH);
    if !path.join("Makefile").exists() {
        println!("cargo:warning=cloning BearSSL");
        Command::new("git")
            .arg("clone")
            .arg("https://www.bearssl.org/git/BearSSL")
            .arg(&path)
            .status()?
            .exit_ok()?;
    } else {
        println!("cargo:warning=fetching BearSSL");
        Command::new("git")
            .arg("fetch")
            .current_dir(&path)
            .status()?
            .exit_ok()?;
    }

    let hash = env::var(BEARSSL_GIT_HASH_VAR);
    let hash = hash.as_deref().unwrap_or(BEARSSL_GIT_HASH);
    Command::new("git")
        .arg("checkout")
        .arg(hash)
        .current_dir(&path)
        .status()?
        .exit_ok()?;

    Ok(Sources::Raw(path))
}

fn find(root: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    glob::glob(root.join(pattern).to_str().unwrap())?
        .collect::<Result<_, _>>()
        .map_err(Into::into)
}

fn main() -> Result<()> {
    let src_dir = match find_bearssl_sources()? {
        Sources::Precompiled(dir) => dir,
        Sources::Raw(dir) => {
            println!("cargo:warning=compiling BearSSL at {:?}", dir);

            cc::Build::new()
                .include(dir.join("inc"))
                .include(dir.join("src"))
                .files(find(&dir, "src/**/*.c")?)
                .opt_level_str("s")
                .compile("bearssl");

            dir
        }
    };

    println!("cargo:rerun-if-env-changed={BEARSSL_INCLUDE_PATH_VAR}");
    let include_path = env::var(BEARSSL_INCLUDE_PATH_VAR)
        .map_or_else(|_| src_dir.join("inc"), |v| Path::new(&v).to_owned());

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
