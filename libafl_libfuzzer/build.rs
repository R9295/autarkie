use core::error::Error;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
};

#[cfg(feature = "rabbit")]
const NAMESPACE: &str = "🐇";
#[cfg(not(feature = "rabbit"))]
const NAMESPACE: &str = "__libafl";
const NAMESPACE_LEN: usize = NAMESPACE.len();

#[expect(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(any(clippy, docsrs)) {
        return Ok(()); // skip when clippy or docs is running
    }

    if cfg!(not(any(target_os = "linux", target_os = "macos"))) {
        println!(
            "cargo:warning=The libafl_libfuzzer runtime may only be built for linux or macos; failing fast."
        );
        return Ok(());
    }

    println!("cargo:rerun-if-changed=libafl_libfuzzer_runtime/src");
    println!("cargo:rerun-if-changed=libafl_libfuzzer_runtime/build.rs");

    let custom_lib_dir =
        AsRef::<Path>::as_ref(&std::env::var_os("OUT_DIR").unwrap()).join("libafl_libfuzzer");
    let custom_lib_target = custom_lib_dir.join("target");
    fs::create_dir_all(&custom_lib_target)
        .expect("Couldn't create the output directory for the fuzzer runtime build");

    let lib_src: PathBuf = AsRef::<Path>::as_ref(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("libafl_libfuzzer_runtime");

    let mut command = Command::new(std::env::var_os("CARGO").unwrap());
    command
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS");

    for (var, _) in std::env::vars() {
        if var.starts_with("CARGO_PKG_") || var.starts_with("CARGO_FEATURE_") {
            command.env_remove(var);
        }
    }

    command
        .env("PATH", std::env::var_os("PATH").unwrap())
        .current_dir(&lib_src);

    command.arg("build");

    let mut features = vec![];

    if cfg!(any(feature = "fork")) {
        features.push("fork");
    }

    if !features.is_empty() {
        command.arg("--features").arg(features.join(","));
    }

    command
        .arg("--release")
        .arg("--no-default-features")
        .arg("--target-dir")
        .arg(&custom_lib_target)
        .arg("--target")
        .arg(std::env::var_os("TARGET").unwrap());

    command.current_dir("../libafl_libfuzzer_runtime");
    // autarkie: make sure we have a grammar source.
    let Ok(grammar_source) = std::env::var("AUTARKIE_GRAMMAR_SRC") else {
        eprintln!("Autarkie: missing path to grammar source (AUTARKIE_GRAMMAR_SRC)");
        panic!("Autarkie: missing path to grammar source (AUTARKIE_GRAMMAR_SRC)");
    };

    let grammar_source = PathBuf::from_str(&grammar_source)?;
    assert!(
        grammar_source.is_absolute(),
        "grammar source must be an absolute path."
    );
    let mut grammar_source_toml =
        toml::from_str(&std::fs::read_to_string(grammar_source.join("Cargo.toml"))?)?;
    let toml::Value::Table(grammar_source_toml) = &mut grammar_source_toml else {
        unreachable!("Invalid Cargo.toml");
    };
    let Some(toml::Value::Table(name)) = grammar_source_toml.get("package") else {
        unreachable!("Invalid Cargo.toml");
    };
    let Some(toml::Value::Table(grammar_deps)) = grammar_source_toml.get("dependencies") else {
        unreachable!("Invalid Cargo.toml");
    };
    let name = name.get("name").unwrap().to_string();

    let mut template = toml::from_str(&std::fs::read_to_string(
        "../libafl_libfuzzer_runtime/Cargo.toml",
    )?)?;
    let toml::Value::Table(root) = &mut template else {
        unreachable!("Invalid Cargo.toml");
    };
    let Some(toml::Value::Table(deps)) = root.get_mut("dependencies") else {
        unreachable!("Invalid Cargo.toml");
    };
    // TODO: remove old grammar
    if deps.contains_key("grammar_source") {
        deps.remove("grammar_source");
    }
    // remove old autarkie dependency
    // We need to re-add it because serialization primives may change
    if deps.contains_key("autarkie") {
        deps.remove("autarkie");
    }
    let mut grammar_autarkie = grammar_deps
        .get("autarkie")
        .expect("Grammar source must have autarkie as a dependency")
        .clone();
    if let Some(autarkie_path) = grammar_autarkie.get("path") {
        assert!(
            PathBuf::from(autarkie_path.to_string().replace("\"", "")).is_absolute(),
            "Autarkie's path must either be absolute or a git repository"
        );
    }
    let Some(toml::Value::Array(autarkie_features)) = grammar_autarkie.get_mut("features") else {
        unreachable!("Invalid autarkie declaration");
    };
    if !autarkie_features.contains(&toml::Value::String("libfuzzer".to_string())) {
        autarkie_features.push("libfuzzer".into());
    }

    let mut dep = toml::map::Map::from_iter([
        (
            "path".to_string(),
            toml::Value::String(grammar_source.to_str().unwrap().to_string()),
        ),
        (
            "package".to_string(),
            toml::Value::String(name.replace("\"", "")),
        ),
    ]);
    if let Ok(features) = std::env::var("AUTARKIE_GRAMMAR_SRC_FEATURES") {
        let features = features.replace(" ", "");
        dep.insert(
            "features".to_string(),
            toml::Value::Array(
                features
                    .split(",")
                    .map(|i| toml::Value::String(i.to_string()))
                    .collect::<Vec<_>>(),
            ),
        );
    }
    deps.insert("grammar_source".to_string(), toml::Value::Table(dep));
    deps.insert("autarkie".to_string(), grammar_autarkie);
    let serialized = toml::to_string(&template)?;
    fs::write("../libafl_libfuzzer_runtime/Cargo.toml", serialized)?;
    assert!(
        command.status().is_ok_and(|s| s.success()),
        "Couldn't build runtime crate! Did you remember to use nightly? (`rustup default nightly` to install)"
    );

    let mut archive_path = custom_lib_target.join(std::env::var_os("TARGET").unwrap());
    archive_path.push("release");

    archive_path.push("libafl_libfuzzer_runtime.a");
    let target_libdir = Command::new("rustc")
        .args(["--print", "target-libdir"])
        .output()
        .expect("Couldn't find rustc's target-libdir");
    let target_libdir = String::from_utf8(target_libdir.stdout).unwrap();
    let target_libdir = Path::new(target_libdir.trim());

    // NOTE: depends on llvm-tools
    let rust_objcopy = target_libdir.join("../bin/llvm-objcopy");
    let nm = target_libdir.join("../bin/llvm-nm");

    let redefined_archive_path = custom_lib_target.join("libFuzzer.a");
    let redefined_symbols = custom_lib_target.join("redefs.txt");

    let mut nm_child = Command::new(nm)
        .arg(&archive_path)
        .stdout(Stdio::piped())
        .spawn()
        .expect("llvm-nm does not work (are you using nightly? or did you install by rustup component add llvm-tools?)");

    let mut redefinitions_file = BufWriter::new(File::create(&redefined_symbols).unwrap());

    let zn_prefix = if cfg!(target_os = "macos") {
        // macOS symbols have an extra `_`
        "__ZN"
    } else {
        "_ZN"
    };

    let replacement = format!("{zn_prefix}{NAMESPACE_LEN}{NAMESPACE}");

    // redefine all the rust-mangled symbols we can
    // TODO this will break when v0 mangling is stabilised
    for line in BufReader::new(nm_child.stdout.take().unwrap()).lines() {
        let line = line.unwrap();

        // Skip headers
        if line.ends_with(':') || line.is_empty() {
            continue;
        }
        let (_, symbol) = line.rsplit_once(' ').unwrap();

        if symbol.starts_with(zn_prefix) {
            writeln!(
                redefinitions_file,
                "{} {}",
                symbol,
                symbol.replacen(zn_prefix, &replacement, 1)
            )
            .unwrap();
        }
    }
    redefinitions_file.flush().unwrap();
    drop(redefinitions_file);

    assert!(
        nm_child.wait().is_ok_and(|s| s.success()),
        "Couldn't link runtime crate! Do you have the llvm-tools component installed? (`rustup component add llvm-tools-preview` to install)"
    );

    let mut objcopy_command = Command::new(rust_objcopy);

    for symbol in [
        "libafl_cmplog_enabled",
        "libafl_cmplog_map",
        "libafl_cmp_map",
        "__rust_drop_panic",
        "__rust_foreign_exception",
        "rust_begin_unwind",
        "rust_panic",
        "rust_eh_personality",
        "__rg_oom",
        "__rdl_oom",
        "__rdl_alloc",
        "__rust_alloc",
        "__rdl_dealloc",
        "__rust_dealloc",
        "__rdl_realloc",
        "__rust_realloc",
        "__rdl_alloc_zeroed",
        "__rust_alloc_zeroed",
        "__rust_alloc_error_handler",
        "__rust_no_alloc_shim_is_unstable",
        "__rust_alloc_error_handler_should_panic",
    ] {
        let mut symbol = symbol.to_string();
        // macOS symbols have an extra `_`
        if cfg!(target_os = "macos") {
            symbol.insert(0, '_');
        }

        objcopy_command
            .arg("--redefine-sym")
            .arg(format!("{symbol}={symbol}_libafl_libfuzzer_runtime"));
    }

    objcopy_command
        .arg("--redefine-syms")
        .arg(redefined_symbols)
        .args([&archive_path, &redefined_archive_path]);

    assert!(
        objcopy_command.status().is_ok_and(|s| s.success()),
        "Couldn't rename allocators in the runtime crate! Do you have the llvm-tools component installed? (`rustup component add llvm-tools-preview` to install)"
    );

    #[cfg(feature = "embed-runtime")]
    {
        // NOTE: lib, .a are added always on unix-like systems as described in:
        // https://gist.github.com/novafacing/1389cbb2f0a362d7eb103e67b4468e2b
        println!(
            "cargo:rustc-env=LIBAFL_LIBFUZZER_RUNTIME_PATH={}",
            redefined_archive_path.display()
        );
    }

    println!(
        "cargo:rustc-link-search=native={}",
        custom_lib_target.to_str().unwrap()
    );
    println!("cargo:rustc-link-lib=static=Fuzzer");

    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=c++");
    } else {
        println!("cargo:rustc-link-lib=stdc++");
    }
    Ok(())
}
